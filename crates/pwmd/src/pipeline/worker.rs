//! OS-thread workers for isolated pipeline queue processing.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

use pwm_core::state::PolicyDecision;
use pwm_core::tx::TxBody;
use pwm_core::{validate_tx_shape, GenCfg, SignedTx};
use tokio::sync::mpsc::{self, error::TryRecvError};

use crate::perfmon;
use crate::pipeline::{
    ClientTxJob, ClusterReadyBatch, DataBroadcastJob, DispatchReceivers, HotIndex, QueueMetrics,
    Receiver, TxRejectReason, ValidatedTx,
};
use crate::state::StateSnapshot;
use tracing::debug_span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffinityQueue {
    ClientTx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRole {
    Affinity(AffinityQueue),
    General,
}

pub struct WorkerPool {
    pub handles: Vec<JoinHandle<()>>,
    pub semaphores: WorkerSemaphores,
}

#[derive(Clone)]
pub struct WorkerCtx {
    reads: WorkerReads,
    validated_tx: mpsc::Sender<ValidatedTx>,
    metrics: Arc<QueueMetrics>,
}

#[derive(Clone)]
pub struct WorkerReads {
    snapshot: Arc<StateSnapshot>,
    hot_index: Arc<HotIndex>,
    cfg: Arc<GenCfg>,
    tip_height: Arc<AtomicU64>,
}

#[derive(Clone)]
pub struct WorkerSemaphores {
    pub client_tx: Arc<Semaphore>,
    pub cluster_ready: Arc<Semaphore>,
    pub data_broadcast: Arc<Semaphore>,
}

pub struct WorkerReceivers {
    client_tx: Mutex<Receiver<ClientTxJob>>,
    cluster_ready: Mutex<Receiver<ClusterReadyBatch>>,
    data_broadcast: Mutex<Receiver<DataBroadcastJob>>,
}

pub struct Semaphore {
    state: Mutex<usize>,
    ready: Condvar,
}

pub struct Permit {
    sem: Arc<Semaphore>,
}

impl WorkerPool {
    pub fn new(
        affinity_count: usize,
        general_count: usize,
        receivers: Arc<WorkerReceivers>,
        ctx: WorkerCtx,
    ) -> Self {
        let semaphores = WorkerSemaphores::new(affinity_count + general_count);
        let mut handles = Vec::with_capacity(affinity_count + general_count);

        for _ in 0..affinity_count {
            handles.push(spawn_worker(
                WorkerRole::Affinity(AffinityQueue::ClientTx),
                Arc::clone(&receivers),
                ctx.clone(),
                semaphores.clone(),
            ));
        }
        for _ in 0..general_count {
            handles.push(spawn_worker(
                WorkerRole::General,
                Arc::clone(&receivers),
                ctx.clone(),
                semaphores.clone(),
            ));
        }

        Self {
            handles,
            semaphores,
        }
    }
}

impl WorkerCtx {
    pub fn new(
        reads: WorkerReads,
        validated_tx: mpsc::Sender<ValidatedTx>,
        metrics: Arc<QueueMetrics>,
    ) -> Self {
        Self {
            reads,
            validated_tx,
            metrics,
        }
    }

    fn snapshot_height(&self) -> u64 {
        self.reads.tip_height.load(Ordering::Relaxed)
    }
}

impl WorkerReads {
    pub fn new(
        snapshot: Arc<StateSnapshot>,
        hot_index: Arc<HotIndex>,
        cfg: Arc<GenCfg>,
        tip_height: Arc<AtomicU64>,
    ) -> Self {
        Self {
            snapshot,
            hot_index,
            cfg,
            tip_height,
        }
    }
}

impl WorkerSemaphores {
    pub fn new(permits: usize) -> Self {
        Self {
            client_tx: Arc::new(Semaphore::new(permits)),
            cluster_ready: Arc::new(Semaphore::new(permits)),
            data_broadcast: Arc::new(Semaphore::new(permits)),
        }
    }
}

impl WorkerReceivers {
    pub fn new(
        client_tx: Receiver<ClientTxJob>,
        cluster_ready: Receiver<ClusterReadyBatch>,
        data_broadcast: Receiver<DataBroadcastJob>,
    ) -> Self {
        Self {
            client_tx: Mutex::new(client_tx),
            cluster_ready: Mutex::new(cluster_ready),
            data_broadcast: Mutex::new(data_broadcast),
        }
    }
}

impl From<DispatchReceivers> for WorkerReceivers {
    fn from(receivers: DispatchReceivers) -> Self {
        Self::new(
            receivers.client_tx,
            receivers.cluster_ready,
            receivers.data_broadcast,
        )
    }
}

impl Semaphore {
    pub fn new(permits: usize) -> Self {
        Self {
            state: Mutex::new(permits),
            ready: Condvar::new(),
        }
    }

    pub fn acquire(self: &Arc<Self>) -> Permit {
        let mut available = self.state.lock().expect("semaphore lock poisoned");
        while *available == 0 {
            available = self.ready.wait(available).expect("semaphore wait poisoned");
        }
        *available -= 1;
        Permit {
            sem: Arc::clone(self),
        }
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut available = self.sem.state.lock().expect("semaphore lock poisoned");
        *available += 1;
        self.sem.ready.notify_one();
    }
}

pub fn spawn_worker(
    role: WorkerRole,
    receivers: Arc<WorkerReceivers>,
    ctx: WorkerCtx,
    sems: WorkerSemaphores,
) -> JoinHandle<()> {
    thread::spawn(move || match role {
        WorkerRole::Affinity(AffinityQueue::ClientTx) => run_client_rx(receivers, ctx, sems),
        WorkerRole::General => run_general_rx(receivers, ctx, sems),
    })
}

fn run_client_rx(receivers: Arc<WorkerReceivers>, ctx: WorkerCtx, sems: WorkerSemaphores) {
    loop {
        let Some(job) = recv_client(&receivers) else {
            break;
        };
        let _permit = sems.client_tx.acquire();
        handle_client(job, &ctx);
    }
}

fn run_general_rx(receivers: Arc<WorkerReceivers>, ctx: WorkerCtx, sems: WorkerSemaphores) {
    loop {
        match try_client(&receivers) {
            Ok(job) => {
                let _permit = sems.client_tx.acquire();
                handle_client(job, &ctx);
                continue;
            }
            Err(TryRecvError::Disconnected) => {}
            Err(TryRecvError::Empty) => {}
        }
        match try_cluster(&receivers) {
            Ok(batch) => {
                let _permit = sems.cluster_ready.acquire();
                handle_cluster(batch);
                continue;
            }
            Err(TryRecvError::Disconnected) => {}
            Err(TryRecvError::Empty) => {}
        }
        match try_broadcast(&receivers) {
            Ok(job) => {
                let _permit = sems.data_broadcast.acquire();
                handle_broadcast(job);
                continue;
            }
            Err(TryRecvError::Disconnected) => {}
            Err(TryRecvError::Empty) => {}
        }
        if all_closed(&receivers) {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn all_closed(receivers: &WorkerReceivers) -> bool {
    let client_closed = receivers
        .client_tx
        .lock()
        .expect("client receiver lock poisoned")
        .is_closed();
    let cluster_closed = receivers
        .cluster_ready
        .lock()
        .expect("cluster receiver lock poisoned")
        .is_closed();
    let broadcast_closed = receivers
        .data_broadcast
        .lock()
        .expect("broadcast receiver lock poisoned")
        .is_closed();
    client_closed && cluster_closed && broadcast_closed
}

fn try_client(receivers: &WorkerReceivers) -> Result<ClientTxJob, TryRecvError> {
    receivers
        .client_tx
        .lock()
        .expect("client receiver lock poisoned")
        .try_recv()
}

fn try_cluster(receivers: &WorkerReceivers) -> Result<ClusterReadyBatch, TryRecvError> {
    receivers
        .cluster_ready
        .lock()
        .expect("cluster receiver lock poisoned")
        .try_recv()
}

fn try_broadcast(receivers: &WorkerReceivers) -> Result<DataBroadcastJob, TryRecvError> {
    receivers
        .data_broadcast
        .lock()
        .expect("broadcast receiver lock poisoned")
        .try_recv()
}

fn recv_client(receivers: &WorkerReceivers) -> Option<ClientTxJob> {
    receivers
        .client_tx
        .lock()
        .expect("client receiver lock poisoned")
        .blocking_recv()
}

fn handle_client(job: ClientTxJob, ctx: &WorkerCtx) {
    ctx.metrics.start_client(job.queue_wait());
    let _span = debug_span!("worker.validate").entered();
    let result = match precheck_client(job.tx.as_ref(), ctx) {
        Ok(validated) => match ctx.validated_tx.try_send(validated) {
            Ok(()) => {
                ctx.metrics.inc_validated();
                Ok(())
            }
            Err(_) => {
                ctx.metrics.inc_rejected();
                Err(TxRejectReason::PrecheckFailed(
                    "validated tx queue is full".to_string(),
                ))
            }
        },
        Err(err) => {
            ctx.metrics.inc_rejected();
            Err(err)
        }
    };
    let _ = job.reply.send(result);
}

fn precheck_client(tx: &SignedTx, ctx: &WorkerCtx) -> Result<ValidatedTx, TxRejectReason> {
    let _span = debug_span!("worker.precheck").entered();
    let sig_scope = perfmon::PERF_ED25519.begin();
    let shape_result = validate_tx_shape(tx);
    sig_scope.end(shape_result.is_ok());
    shape_result.map_err(TxRejectReason::ShapeInvalid)?;
    let snapshot_height = ctx.snapshot_height();
    if let Some(result) = precheck_hot(tx, ctx) {
        result?;
        return Ok(validated_tx(tx, snapshot_height));
    }
    precheck_full(tx, ctx, snapshot_height)?;
    Ok(validated_tx(tx, snapshot_height))
}

fn precheck_hot(tx: &SignedTx, ctx: &WorkerCtx) -> Option<Result<(), TxRejectReason>> {
    let TxBody::Transfer { to, amount, fee } = &tx.body else {
        return None;
    };
    let accounts = ctx.reads.hot_index.load();
    let sender = accounts.get(&tx.computed_account_id())?;
    let recipient = accounts.get(to)?;
    if !hot_safe(sender) || !hot_safe(recipient) {
        return None;
    }
    Some(check_hot_transfer(
        tx.nonce, *amount, *fee, sender, recipient,
    ))
}

fn hot_safe(account: &crate::pipeline::AccountHot) -> bool {
    account.flags == 0 && account.active_policies == 0
}

fn check_hot_transfer(
    nonce: u64,
    amount: u128,
    fee: u128,
    sender: &crate::pipeline::AccountHot,
    recipient: &crate::pipeline::AccountHot,
) -> Result<(), TxRejectReason> {
    if !sender.initialized {
        return Err(precheck_msg("sender account is not initialized"));
    }
    if !recipient.initialized {
        return Err(precheck_msg("recipient account is not initialized"));
    }
    if nonce != sender.nonce {
        return Err(TxRejectReason::StaleDuplicate);
    }
    let total = amount
        .checked_add(fee)
        .ok_or_else(|| precheck_msg("transfer amount plus fee overflow"))?;
    if sender.balance < total {
        return Err(precheck_msg("insufficient balance"));
    }
    Ok(())
}

fn precheck_full(
    tx: &SignedTx,
    ctx: &WorkerCtx,
    snapshot_height: u64,
) -> Result<(), TxRejectReason> {
    let state = ctx.reads.snapshot.load();
    if !matches!(tx.body, TxBody::Init { .. }) {
        match state.evaluate_policy(tx, snapshot_height) {
            PolicyDecision::Allow => {}
            PolicyDecision::Reject(_) | PolicyDecision::Redirect(_) => {
                return Err(TxRejectReason::PolicyDenied);
            }
        }
    }
    let next_h = snapshot_height.saturating_add(1);
    let next_ts = unix_now_secs()?;
    state
        .precheck_apply_with_ctx(tx, next_h, next_ts, &ctx.reads.cfg)
        .map_err(precheck_err)
}

fn validated_tx(tx: &SignedTx, snapshot_height: u64) -> ValidatedTx {
    ValidatedTx {
        tx: tx.clone(),
        validated_at_height: snapshot_height,
    }
}

fn precheck_msg(reason: &str) -> TxRejectReason {
    TxRejectReason::PrecheckFailed(reason.to_string())
}

fn unix_now_secs() -> Result<u64, TxRejectReason> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|err| TxRejectReason::PrecheckFailed(err.to_string()))
}

fn precheck_err(err: pwm_core::tx::TxError) -> TxRejectReason {
    use pwm_core::tx::TxError::{AlreadyInit, BadNonce, DuplicateImport};
    match err {
        BadNonce | DuplicateImport | AlreadyInit => TxRejectReason::StaleDuplicate,
        other => TxRejectReason::PrecheckFailed(other.to_string()),
    }
}

fn handle_cluster(batch: ClusterReadyBatch) {
    drop(batch);
}

fn handle_broadcast(job: DataBroadcastJob) {
    drop(job);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{dispatch, BoundedQueue, DispatchInput};
    use crate::state::StateSnapshot;
    use ed25519_dalek::SigningKey;
    use pwm_core::{
        hd::{account_id_from_parts, domain_of_account_id},
        tx::{PolicyKind, TxBody},
        types::{address_flags, Account, CONSERVATION, COSIGN_NON_DISABLEABLE},
        AccountId, Chain, SignedTx,
    };
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;
    use tokio::sync::oneshot;

    fn init_tx(nonce: u64) -> SignedTx {
        let sk = SigningKey::from_bytes(&[11u8; 32]);
        let probe = SignedTx::sign_body(&sk, 0, 0, nonce, TxBody::Stake { amount: 1 });
        let dom = domain_of_account_id(&probe.computed_account_id());
        SignedTx::sign_body(&sk, dom, 0, nonce, TxBody::Init { index: 1, flags: 0 })
    }

    const TEST_BALANCE: u128 = 100;
    const POLICY_FLAGS: u32 = COSIGN_NON_DISABLEABLE | CONSERVATION;

    struct TestAccounts {
        sender_sk: SigningKey,
        sender: AccountId,
        recipient: AccountId,
        recipient_pk: [u8; 32],
        balance: u128,
    }

    fn clean_signer(start: u16) -> SigningKey {
        for candidate in start..=u16::MAX {
            let mut seed = [0xA5; 32];
            seed[..2].copy_from_slice(&candidate.to_le_bytes());
            let sk = SigningKey::from_bytes(&seed);
            let id = account_id_from_parts(&sk.verifying_key().to_bytes(), 0);
            if address_flags(&id) & POLICY_FLAGS == 0 {
                return sk;
            }
        }
        panic!("clean test signer not found")
    }

    fn test_accounts() -> TestAccounts {
        let sender_sk = clean_signer(0);
        let recipient_sk = clean_signer(u16::MAX / 2);
        let sender = account_id_from_parts(&sender_sk.verifying_key().to_bytes(), 0);
        let recipient_pk = recipient_sk.verifying_key().to_bytes();
        let recipient = account_id_from_parts(&recipient_pk, 0);
        assert_ne!(sender, recipient);
        assert_eq!(address_flags(&sender) & POLICY_FLAGS, 0);
        assert_eq!(address_flags(&recipient) & POLICY_FLAGS, 0);
        TestAccounts {
            sender_sk,
            sender,
            recipient,
            recipient_pk,
            balance: TEST_BALANCE,
        }
    }

    fn test_transfer(accounts: &TestAccounts, nonce: u64, amount: u128) -> SignedTx {
        let dom = domain_of_account_id(&accounts.sender);
        SignedTx::sign_body(
            &accounts.sender_sk,
            dom,
            0,
            nonce,
            TxBody::Transfer {
                to: accounts.recipient,
                amount,
                fee: 0,
            },
        )
    }

    fn client_job(tx: SignedTx) -> (ClientTxJob, oneshot::Receiver<Result<(), TxRejectReason>>) {
        let (reply, rx) = oneshot::channel();
        (ClientTxJob::new(Arc::new(tx), reply), rx)
    }

    fn test_ctx() -> (WorkerCtx, mpsc::Receiver<ValidatedTx>, TestAccounts) {
        let (cfg, sks) = pwm_core::dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks);
        let accounts = test_accounts();
        chain.st.accounts.insert(
            accounts.sender,
            Account::genesis_funded(
                accounts.sender_sk.verifying_key().to_bytes(),
                0,
                accounts.balance,
            ),
        );
        chain.st.accounts.insert(
            accounts.recipient,
            Account::genesis_funded(accounts.recipient_pk, 0, 0),
        );
        let hot_index = Arc::new(HotIndex::new(&chain.st));
        let snapshot = Arc::new(StateSnapshot::new(Arc::new(chain.st)));
        let (tx, rx) = mpsc::channel(4);
        let reads = WorkerReads::new(
            snapshot,
            hot_index,
            Arc::new(cfg.clone()),
            Arc::new(AtomicU64::new(0)),
        );
        let ctx = WorkerCtx::new(reads, tx, Arc::new(QueueMetrics::default()));
        (ctx, rx, accounts)
    }

    #[test]
    fn test_worker_client_tx() {
        let (queues, receivers) = crate::pipeline::DispatchQueues::new_with_workers(2, 2, 2);
        let receivers = Arc::new(receivers);
        let sems = WorkerSemaphores::new(1);
        let (ctx, mut valid_rx, accounts) = test_ctx();
        let tx = test_transfer(&accounts, 0, 1);
        assert_eq!(precheck_hot(&tx, &ctx), Some(Ok(())));
        let handle = spawn_worker(
            WorkerRole::Affinity(AffinityQueue::ClientTx),
            Arc::clone(&receivers),
            ctx,
            sems,
        );
        let tx_nonce = tx.nonce;
        let (job, rx) = client_job(tx);

        dispatch(&queues, DispatchInput::ClientTx(job)).expect("dispatch client job");
        drop(queues);

        assert_eq!(rx.blocking_recv().expect("worker reply"), Ok(()));
        let valid = valid_rx.blocking_recv().expect("validated tx");
        assert_eq!(valid.validated_at_height, 0);
        assert_eq!(valid.tx.nonce, tx_nonce);
        handle.join().expect("worker exits");
    }

    #[test]
    fn test_worker_accepts_init() {
        let (queues, receivers) = crate::pipeline::DispatchQueues::new_with_workers(2, 2, 2);
        let receivers = Arc::new(receivers);
        let (ctx, mut valid_rx, _accounts) = test_ctx();
        let handle = spawn_worker(
            WorkerRole::Affinity(AffinityQueue::ClientTx),
            Arc::clone(&receivers),
            ctx,
            WorkerSemaphores::new(1),
        );
        let tx = init_tx(0);
        let (job, rx) = client_job(tx);

        dispatch(&queues, DispatchInput::ClientTx(job)).expect("dispatch init job");
        drop(queues);

        assert_eq!(rx.blocking_recv().expect("worker reply"), Ok(()));
        let valid = valid_rx.blocking_recv().expect("validated init tx");
        assert!(matches!(valid.tx.body, TxBody::Init { .. }));
        handle.join().expect("worker exits");
    }

    #[test]
    fn test_affinity_only_client() {
        let (queues, receivers) = crate::pipeline::DispatchQueues::new_with_workers(2, 2, 2);
        let receivers = Arc::new(receivers);
        let (ctx, _valid_rx, accounts) = test_ctx();
        let handle = spawn_worker(
            WorkerRole::Affinity(AffinityQueue::ClientTx),
            Arc::clone(&receivers),
            ctx,
            WorkerSemaphores::new(1),
        );
        let (job, rx) = client_job(test_transfer(&accounts, 0, 1));

        dispatch(
            &queues,
            DispatchInput::ClusterReady(ClusterReadyBatch {
                txs: vec![init_tx(10)],
            }),
        )
        .expect("dispatch cluster batch");
        dispatch(&queues, DispatchInput::ClientTx(job)).expect("dispatch client job");
        drop(queues);

        assert_eq!(rx.blocking_recv().expect("worker reply"), Ok(()));
        handle.join().expect("worker exits");
        let mut cluster = receivers
            .cluster_ready
            .lock()
            .expect("cluster receiver lock poisoned");
        assert_eq!(
            cluster
                .try_recv()
                .expect("cluster remains queued")
                .txs
                .len(),
            1
        );
    }

    #[test]
    fn test_backpressure_rejects() {
        let (queue, _rx) = BoundedQueue::new(2);
        let sem = Arc::new(Semaphore::new(2));
        let _permit_a = sem.acquire();
        let _permit_b = sem.acquire();
        let (job_a, _rx_a) = client_job(init_tx(0));
        let (job_b, _rx_b) = client_job(init_tx(1));
        let (job_c, _rx_c) = client_job(init_tx(2));

        assert!(queue.try_push(job_a).is_ok());
        assert!(queue.try_push(job_b).is_ok());
        assert!(queue.try_push(job_c).is_err());

        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(queue.metrics().rejected, 1);
    }

    #[test]
    fn worker_rejects_bad_nonce() {
        let (queues, receivers) = crate::pipeline::DispatchQueues::new_with_workers(2, 2, 2);
        let receivers = Arc::new(receivers);
        let (ctx, _valid_rx, accounts) = test_ctx();
        let bad_tx = test_transfer(&accounts, 1, 1);
        assert_eq!(
            precheck_hot(&bad_tx, &ctx),
            Some(Err(TxRejectReason::StaleDuplicate))
        );
        let handle = spawn_worker(
            WorkerRole::Affinity(AffinityQueue::ClientTx),
            Arc::clone(&receivers),
            ctx,
            WorkerSemaphores::new(1),
        );
        let (job, rx) = client_job(bad_tx);

        dispatch(&queues, DispatchInput::ClientTx(job)).expect("dispatch client job");
        drop(queues);

        assert_eq!(
            rx.blocking_recv().expect("worker reply"),
            Err(TxRejectReason::StaleDuplicate)
        );
        handle.join().expect("worker exits");
    }

    #[test]
    fn worker_rejects_low_balance() {
        let (queues, receivers) = crate::pipeline::DispatchQueues::new_with_workers(2, 2, 2);
        let (ctx, _valid_rx, accounts) = test_ctx();
        let amount = accounts.balance.saturating_add(1);
        let tx = test_transfer(&accounts, 0, amount);
        assert_eq!(
            precheck_hot(&tx, &ctx),
            Some(Err(precheck_msg("insufficient balance")))
        );
        let handle = spawn_worker(
            WorkerRole::Affinity(AffinityQueue::ClientTx),
            Arc::new(receivers),
            ctx,
            WorkerSemaphores::new(1),
        );
        let (job, rx) = client_job(tx);

        dispatch(&queues, DispatchInput::ClientTx(job)).expect("dispatch client job");
        drop(queues);

        assert_eq!(
            rx.blocking_recv().expect("worker reply"),
            Err(precheck_msg("insufficient balance"))
        );
        handle.join().expect("worker exits");
    }

    #[test]
    fn worker_policy_uses_fallback() {
        let (queues, receivers) = crate::pipeline::DispatchQueues::new_with_workers(2, 2, 2);
        let (ctx, _valid_rx, accounts) = test_ctx();
        let mut state = (*ctx.reads.snapshot.load()).clone();
        state
            .accounts
            .get_mut(&accounts.recipient)
            .expect("recipient")
            .active_policies = PolicyKind::DefaultBehavior.bit();
        ctx.reads.hot_index.refresh(&state);
        ctx.reads.snapshot.store(Arc::new(state));
        let tx = test_transfer(&accounts, 0, 1);
        assert!(precheck_hot(&tx, &ctx).is_none());
        let handle = spawn_worker(
            WorkerRole::Affinity(AffinityQueue::ClientTx),
            Arc::new(receivers),
            ctx,
            WorkerSemaphores::new(1),
        );
        let (job, rx) = client_job(tx);

        dispatch(&queues, DispatchInput::ClientTx(job)).expect("dispatch client job");
        drop(queues);

        assert_eq!(
            rx.blocking_recv().expect("worker reply"),
            Err(TxRejectReason::PolicyDenied)
        );
        handle.join().expect("worker exits");
    }
}
