//! Bounded pipeline queues and owned job contracts.

use pwm_core::{tx::TxError, SignedTx};
use serde::Serialize;
use std::{
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};

pub struct ClientTxJob {
    pub tx: Arc<SignedTx>,
    pub reply: oneshot::Sender<Result<(), TxRejectReason>>,
    queued_at: Instant,
}

impl ClientTxJob {
    pub fn new(tx: Arc<SignedTx>, reply: oneshot::Sender<Result<(), TxRejectReason>>) -> Self {
        Self {
            tx,
            reply,
            queued_at: Instant::now(),
        }
    }

    pub(crate) fn queue_wait(&self) -> Duration {
        self.queued_at.elapsed()
    }
}

pub struct ClusterReadyBatch {
    pub txs: Vec<SignedTx>,
}

pub struct TxIngressChannel {
    pub sender: mpsc::Sender<SignedTx>,
    pub receiver: tokio::sync::Mutex<mpsc::Receiver<SignedTx>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxOrigin {
    DirectHttp,
    HelperNode { helper_id: u16, batch_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxEntryState {
    Pending,
    Validated { at_height: u64 },
    Sealed { block_height: u64 },
    Rejected { reason: TxRejectReason },
}

#[derive(Debug, Clone)]
pub struct TxEntry {
    pub tx: SignedTx,
    pub ingress_height: u64,
    pub state: TxEntryState,
    pub origin: TxOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxRejectReason {
    ShapeInvalid(TxError),
    PolicyDenied,
    PrecheckFailed(String),
    StaleDuplicate,
}

impl fmt::Display for TxRejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShapeInvalid(reason) => write!(f, "shape invalid: {reason}"),
            Self::PolicyDenied => f.write_str("policy denied"),
            Self::PrecheckFailed(reason) => write!(f, "precheck failed: {reason}"),
            Self::StaleDuplicate => f.write_str("stale duplicate"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedTx {
    pub tx: SignedTx,
    pub validated_at_height: u64,
}

#[derive(Debug, Clone)]
pub enum TxEvent {
    Sealed {
        txid: [u8; 32],
        block_height: u64,
    },
    Rejected {
        txid: [u8; 32],
        reason: TxRejectReason,
    },
}

impl TxIngressChannel {
    pub fn new(cap: usize) -> Self {
        let (sender, receiver) = mpsc::channel(cap);
        Self {
            sender,
            receiver: tokio::sync::Mutex::new(receiver),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataBroadcastJob {
    pub topic: String,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct QueueMetrics {
    enqueued: AtomicU64,
    dequeued: AtomicU64,
    rejected: AtomicU64,
    validated: AtomicU64,
    stale_validated: AtomicU64,
    queue_depth: AtomicU64,
    queue_depth_max: AtomicU64,
    last_depth_max: AtomicU64,
    worker_wait: [AtomicU64; WAIT_BUCKETS],
}

const WAIT_BUCKETS: usize = 64;

impl Default for QueueMetrics {
    fn default() -> Self {
        Self {
            enqueued: AtomicU64::new(0),
            dequeued: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            validated: AtomicU64::new(0),
            stale_validated: AtomicU64::new(0),
            queue_depth: AtomicU64::new(0),
            queue_depth_max: AtomicU64::new(0),
            last_depth_max: AtomicU64::new(u64::MAX),
            worker_wait: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct QueueMetricsSnapshot {
    pub enqueued: u64,
    pub dequeued: u64,
    pub rejected: u64,
    pub validated: u64,
    pub stale_validated: u64,
    pub queue_depth_max: u64,
    pub worker_wait_p50_ms: u64,
}

pub type Receiver<T> = BoundedQueueReceiver<T>;

pub struct BoundedQueue<T> {
    sender: mpsc::Sender<T>,
    enqueued: Arc<AtomicU64>,
    rejected: Arc<AtomicU64>,
    dequeued: Arc<AtomicU64>,
}

pub struct BoundedQueueReceiver<T> {
    receiver: mpsc::Receiver<T>,
    dequeued: Arc<AtomicU64>,
}

impl<T> Clone for BoundedQueue<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            enqueued: Arc::clone(&self.enqueued),
            rejected: Arc::clone(&self.rejected),
            dequeued: Arc::clone(&self.dequeued),
        }
    }
}

impl<T> BoundedQueue<T> {
    pub fn new(cap: usize) -> (Self, Receiver<T>) {
        let (sender, receiver) = mpsc::channel(cap);
        let dequeued = Arc::new(AtomicU64::new(0));
        let queue = Self {
            sender,
            enqueued: Arc::new(AtomicU64::new(0)),
            rejected: Arc::new(AtomicU64::new(0)),
            dequeued: Arc::clone(&dequeued),
        };
        let receiver = BoundedQueueReceiver { receiver, dequeued };
        (queue, receiver)
    }

    pub fn try_push(&self, item: T) -> Result<(), T> {
        match self.sender.try_send(item) {
            Ok(()) => {
                self.enqueued.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(item))
            | Err(mpsc::error::TrySendError::Closed(item)) => {
                self.rejected.fetch_add(1, Ordering::Relaxed);
                Err(item)
            }
        }
    }

    pub fn metrics(&self) -> QueueMetricsSnapshot {
        QueueMetricsSnapshot {
            enqueued: self.enqueued.load(Ordering::Relaxed),
            dequeued: self.dequeued.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            validated: 0,
            stale_validated: 0,
            queue_depth_max: 0,
            worker_wait_p50_ms: 0,
        }
    }
}

impl QueueMetrics {
    pub fn inc_enqueued(&self) {
        self.enqueued.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_dequeued(&self) {
        self.dequeued.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_rejected(&self) {
        self.rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_validated(&self) {
        self.validated.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_stale_validated(&self) {
        self.stale_validated.fetch_add(1, Ordering::Relaxed);
    }

    pub fn start_dispatch(&self) -> u64 {
        self.queue_depth.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn commit_dispatch(&self, depth: u64) {
        self.queue_depth_max.fetch_max(depth, Ordering::Relaxed);
    }

    pub fn cancel_dispatch(&self) {
        self.dec_queue_depth();
    }

    pub fn start_client(&self, wait: Duration) {
        self.dec_queue_depth();
        let wait_ms = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX);
        let bucket = wait_bucket(wait_ms);
        self.worker_wait[bucket].fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn finish_block(&self) {
        let depth = self.queue_depth.load(Ordering::Acquire);
        let completed = self.queue_depth_max.swap(depth, Ordering::AcqRel);
        self.last_depth_max.store(completed, Ordering::Release);
    }

    fn dec_queue_depth(&self) {
        let _ = self
            .queue_depth
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
                Some(depth.saturating_sub(1))
            });
    }

    fn worker_wait_p50_ms(&self) -> u64 {
        let total = self
            .worker_wait
            .iter()
            .map(|bucket| bucket.load(Ordering::Relaxed))
            .sum::<u64>();
        if total == 0 {
            return 0;
        }
        let target = total.div_ceil(2);
        let mut seen = 0;
        for (ms, bucket) in self.worker_wait.iter().enumerate() {
            seen += bucket.load(Ordering::Relaxed);
            if seen >= target {
                return wait_bound_ms(ms);
            }
        }
        wait_bound_ms(WAIT_BUCKETS - 1)
    }

    pub fn snapshot(&self) -> QueueMetricsSnapshot {
        let last_max = self.last_depth_max.load(Ordering::Acquire);
        let depth_max = if last_max == u64::MAX {
            self.queue_depth_max.load(Ordering::Relaxed)
        } else {
            last_max
        };
        QueueMetricsSnapshot {
            enqueued: self.enqueued.load(Ordering::Relaxed),
            dequeued: self.dequeued.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            validated: self.validated.load(Ordering::Relaxed),
            stale_validated: self.stale_validated.load(Ordering::Relaxed),
            queue_depth_max: depth_max,
            worker_wait_p50_ms: self.worker_wait_p50_ms(),
        }
    }
}

fn wait_bucket(wait_ms: u64) -> usize {
    if wait_ms == 0 {
        return 0;
    }
    let bucket = u64::BITS - wait_ms.leading_zeros();
    usize::try_from(bucket)
        .unwrap_or(WAIT_BUCKETS - 1)
        .min(WAIT_BUCKETS - 1)
}

fn wait_bound_ms(bucket: usize) -> u64 {
    1u64.checked_shl(u32::try_from(bucket).unwrap_or(u32::MAX))
        .unwrap_or(u64::MAX)
        .saturating_sub(1)
}

impl<T> BoundedQueueReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        let item = self.receiver.recv().await;
        if item.is_some() {
            self.dequeued.fetch_add(1, Ordering::Relaxed);
        }
        item
    }

    pub fn blocking_recv(&mut self) -> Option<T> {
        let item = self.receiver.blocking_recv();
        if item.is_some() {
            self.dequeued.fetch_add(1, Ordering::Relaxed);
        }
        item
    }

    pub fn try_recv(&mut self) -> Result<T, mpsc::error::TryRecvError> {
        let item = self.receiver.try_recv();
        if item.is_ok() {
            self.dequeued.fetch_add(1, Ordering::Relaxed);
        }
        item
    }

    pub fn is_closed(&self) -> bool {
        self.receiver.is_closed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use pwm_core::tx::TxBody;

    fn test_tx(nonce: u64) -> SignedTx {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        SignedTx::sign_body(&sk, 0x2C01, 0, nonce, TxBody::Init { index: 1, flags: 0 })
    }

    #[test]
    fn test_queue_rejection_on_full() {
        let (queue, _rx) = BoundedQueue::new(1);

        assert!(queue.try_push(1u8).is_ok());
        assert_eq!(queue.try_push(2u8), Err(2u8));

        assert_eq!(
            queue.metrics(),
            QueueMetricsSnapshot {
                enqueued: 1,
                dequeued: 0,
                rejected: 1,
                validated: 0,
                stale_validated: 0,
                queue_depth_max: 0,
                worker_wait_p50_ms: 0,
            }
        );
    }

    #[test]
    fn test_queue_metrics() {
        let (queue, mut rx) = BoundedQueue::new(3);

        queue.try_push(test_tx(0)).expect("enqueue tx 0");
        queue.try_push(test_tx(1)).expect("enqueue tx 1");
        queue.try_push(test_tx(2)).expect("enqueue tx 2");

        assert!(rx.try_recv().is_ok());
        assert!(rx.try_recv().is_ok());

        assert_eq!(
            queue.metrics(),
            QueueMetricsSnapshot {
                enqueued: 3,
                dequeued: 2,
                rejected: 0,
                validated: 0,
                stale_validated: 0,
                queue_depth_max: 0,
                worker_wait_p50_ms: 0,
            }
        );
    }

    #[test]
    fn test_pipeline_depth_wait() {
        let metrics = QueueMetrics::default();
        let first = metrics.start_dispatch();
        metrics.commit_dispatch(first);
        let second = metrics.start_dispatch();
        metrics.commit_dispatch(second);
        metrics.start_client(Duration::from_millis(2));
        metrics.start_client(Duration::from_millis(150));

        let third = metrics.start_dispatch();
        metrics.commit_dispatch(third);
        metrics.start_client(Duration::from_millis(500));

        let snap = metrics.snapshot();
        assert_eq!(snap.queue_depth_max, 2);
        assert_eq!(snap.worker_wait_p50_ms, 255);

        metrics.finish_block();
        assert_eq!(metrics.snapshot().queue_depth_max, 2);

        let next = metrics.start_dispatch();
        metrics.commit_dispatch(next);
        metrics.start_client(Duration::from_millis(3));
        metrics.finish_block();
        assert_eq!(metrics.snapshot().queue_depth_max, 1);
    }

    #[test]
    fn test_dispatch_cancel_depth() {
        let metrics = QueueMetrics::default();
        let depth = metrics.start_dispatch();
        metrics.cancel_dispatch();

        assert_eq!(depth, 1);
        assert_eq!(metrics.snapshot().queue_depth_max, 0);
        assert_eq!(metrics.snapshot().worker_wait_p50_ms, 0);
    }
}
