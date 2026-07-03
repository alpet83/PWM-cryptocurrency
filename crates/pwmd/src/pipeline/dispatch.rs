//! Dispatch routing for the three isolated pipeline ingress queues.

use crate::pipeline::queue::{
    BoundedQueue, ClientTxJob, ClusterReadyBatch, DataBroadcastJob, Receiver,
};

pub struct DispatchQueues {
    pub client_tx: BoundedQueue<ClientTxJob>,
    pub cluster_ready: BoundedQueue<ClusterReadyBatch>,
    pub data_broadcast: BoundedQueue<DataBroadcastJob>,
}

pub struct DispatchReceivers {
    pub client_tx: Receiver<ClientTxJob>,
    pub cluster_ready: Receiver<ClusterReadyBatch>,
    pub data_broadcast: Receiver<DataBroadcastJob>,
}

pub enum DispatchInput {
    ClientTx(ClientTxJob),
    ClusterReady(ClusterReadyBatch),
    DataBroadcast(DataBroadcastJob),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError {
    ClientTxFull,
    ClusterReadyFull,
    DataBroadcastFull,
}

impl DispatchQueues {
    pub fn new(client_cap: usize, cluster_cap: usize, broadcast_cap: usize) -> Self {
        Self::new_with_receivers(client_cap, cluster_cap, broadcast_cap).0
    }

    pub fn new_with_receivers(
        client_cap: usize,
        cluster_cap: usize,
        broadcast_cap: usize,
    ) -> (Self, DispatchReceivers) {
        let (client_tx, client_tx_rx) = BoundedQueue::new(client_cap);
        let (cluster_ready, cluster_ready_rx) = BoundedQueue::new(cluster_cap);
        let (data_broadcast, data_broadcast_rx) = BoundedQueue::new(broadcast_cap);
        (
            Self {
                client_tx,
                cluster_ready,
                data_broadcast,
            },
            DispatchReceivers {
                client_tx: client_tx_rx,
                cluster_ready: cluster_ready_rx,
                data_broadcast: data_broadcast_rx,
            },
        )
    }

    pub fn new_with_workers(
        client_cap: usize,
        cluster_cap: usize,
        broadcast_cap: usize,
    ) -> (Self, crate::pipeline::worker::WorkerReceivers) {
        let (queues, receivers) = Self::new_with_receivers(client_cap, cluster_cap, broadcast_cap);
        (
            queues,
            crate::pipeline::worker::WorkerReceivers::from(receivers),
        )
    }
}

pub fn dispatch(queues: &DispatchQueues, input: DispatchInput) -> Result<(), DispatchError> {
    match input {
        DispatchInput::ClientTx(job) => queues
            .client_tx
            .try_push(job)
            .map_err(|_| DispatchError::ClientTxFull),
        DispatchInput::ClusterReady(batch) => queues
            .cluster_ready
            .try_push(batch)
            .map_err(|_| DispatchError::ClusterReadyFull),
        DispatchInput::DataBroadcast(job) => queues
            .data_broadcast
            .try_push(job)
            .map_err(|_| DispatchError::DataBroadcastFull),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use pwm_core::{tx::TxBody, SignedTx};
    use std::sync::Arc;
    use tokio::sync::oneshot;

    fn test_tx(nonce: u64) -> SignedTx {
        let sk = SigningKey::from_bytes(&[9u8; 32]);
        SignedTx::sign_body(&sk, 0x2C01, 0, nonce, TxBody::Init { index: 1, flags: 0 })
    }

    fn client_job(nonce: u64) -> ClientTxJob {
        let (reply, _rx) = oneshot::channel();
        ClientTxJob::new(Arc::new(test_tx(nonce)), reply)
    }

    #[test]
    fn test_dispatch_client_tx() {
        let (queues, mut receivers) = DispatchQueues::new_with_receivers(1, 1, 1);

        dispatch(&queues, DispatchInput::ClientTx(client_job(0))).expect("dispatch client tx");

        let received = receivers.client_tx.try_recv().expect("receive client tx");
        assert_eq!(received.tx.nonce, 0);
        assert_eq!(queues.client_tx.metrics().enqueued, 1);
        assert_eq!(queues.client_tx.metrics().dequeued, 1);
        assert_eq!(queues.cluster_ready.metrics().enqueued, 0);
        assert_eq!(queues.data_broadcast.metrics().enqueued, 0);
    }

    #[test]
    fn test_dispatch_cluster_ready() {
        let (queues, mut receivers) = DispatchQueues::new_with_receivers(1, 1, 1);

        dispatch(
            &queues,
            DispatchInput::ClusterReady(ClusterReadyBatch {
                txs: vec![test_tx(10), test_tx(11)],
            }),
        )
        .expect("dispatch cluster ready");

        let received = receivers
            .cluster_ready
            .try_recv()
            .expect("receive cluster ready");
        assert_eq!(received.txs.len(), 2);
        assert_eq!(received.txs[0].nonce, 10);
        assert_eq!(queues.cluster_ready.metrics().enqueued, 1);
        assert_eq!(queues.cluster_ready.metrics().dequeued, 1);
        assert_eq!(queues.client_tx.metrics().enqueued, 0);
        assert_eq!(queues.data_broadcast.metrics().enqueued, 0);
    }

    #[test]
    fn test_dispatch_data_broadcast() {
        let (queues, mut receivers) = DispatchQueues::new_with_receivers(1, 1, 1);

        dispatch(
            &queues,
            DispatchInput::DataBroadcast(DataBroadcastJob {
                topic: "blocks".to_string(),
                payload: vec![1, 2, 3],
            }),
        )
        .expect("dispatch data broadcast");

        let received = receivers
            .data_broadcast
            .try_recv()
            .expect("receive data broadcast");
        assert_eq!(received.topic, "blocks");
        assert_eq!(received.payload, vec![1, 2, 3]);
        assert_eq!(queues.data_broadcast.metrics().enqueued, 1);
        assert_eq!(queues.data_broadcast.metrics().dequeued, 1);
        assert_eq!(queues.client_tx.metrics().enqueued, 0);
        assert_eq!(queues.cluster_ready.metrics().enqueued, 0);
    }

    #[test]
    fn test_dispatch_client_full() {
        let (queues, _receivers) = DispatchQueues::new_with_receivers(1, 1, 1);

        dispatch(&queues, DispatchInput::ClientTx(client_job(0))).expect("first client tx");
        let err = dispatch(&queues, DispatchInput::ClientTx(client_job(1)))
            .expect_err("client tx queue must reject when full");

        assert_eq!(err, DispatchError::ClientTxFull);
        assert_eq!(queues.client_tx.metrics().rejected, 1);
    }

    #[test]
    fn test_dispatch_cluster_full() {
        let (queues, _receivers) = DispatchQueues::new_with_receivers(1, 1, 1);

        let batch = ClusterReadyBatch {
            txs: vec![test_tx(20)],
        };
        dispatch(&queues, DispatchInput::ClusterReady(batch)).expect("first cluster batch");
        let err = dispatch(
            &queues,
            DispatchInput::ClusterReady(ClusterReadyBatch {
                txs: vec![test_tx(21)],
            }),
        )
        .expect_err("cluster queue must reject when full");

        assert_eq!(err, DispatchError::ClusterReadyFull);
        assert_eq!(queues.cluster_ready.metrics().rejected, 1);
    }

    #[test]
    fn test_dispatch_broadcast_full() {
        let (queues, _receivers) = DispatchQueues::new_with_receivers(1, 1, 1);

        let first = DataBroadcastJob {
            topic: "blocks".to_string(),
            payload: vec![1],
        };
        dispatch(&queues, DispatchInput::DataBroadcast(first)).expect("first broadcast");
        let err = dispatch(
            &queues,
            DispatchInput::DataBroadcast(DataBroadcastJob {
                topic: "blocks".to_string(),
                payload: vec![2],
            }),
        )
        .expect_err("broadcast queue must reject when full");

        assert_eq!(err, DispatchError::DataBroadcastFull);
        assert_eq!(queues.data_broadcast.metrics().rejected, 1);
    }
}
