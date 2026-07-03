//! Global monotonic transaction counters for HTTP backpressure.

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

pub static TX_COUNTER_INCOMING: AtomicU64 = AtomicU64::new(0);
pub static TX_COUNTER_SEALED: AtomicU64 = AtomicU64::new(0);
pub static TX_COUNTER_REJECTED: AtomicU64 = AtomicU64::new(0);

/// Monotonic transaction counters for client-side backpressure.
/// `incoming` is HTTP ingress, `sealed + rejected <= incoming`, and the gap is queued in-flight work.
/// Snapshot ratios are approximate because fields are loaded independently.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct TxCounters {
    pub incoming: u64,
    pub sealed: u64,
    pub rejected: u64,
}

pub fn snapshot() -> TxCounters {
    TxCounters {
        incoming: TX_COUNTER_INCOMING.load(Ordering::Relaxed),
        sealed: TX_COUNTER_SEALED.load(Ordering::Relaxed),
        rejected: TX_COUNTER_REJECTED.load(Ordering::Relaxed),
    }
}

pub fn inc_incoming() {
    TX_COUNTER_INCOMING.fetch_add(1, Ordering::Relaxed);
}

pub fn inc_sealed() {
    inc_sealed_by(1);
}

pub fn inc_sealed_by(count: u64) {
    TX_COUNTER_SEALED.fetch_add(count, Ordering::Relaxed);
}

pub fn inc_rejected() {
    inc_rejected_by(1);
}

pub fn inc_rejected_by(count: u64) {
    TX_COUNTER_REJECTED.fetch_add(count, Ordering::Relaxed);
}
