//! Lightweight process-local performance counters for named hot-path scopes.
#![allow(dead_code)]

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Monotonic counters for one performance-monitored code path.
pub(crate) struct PerfEntity {
    name: &'static str,
    calls: AtomicU64,
    success: AtomicU64,
    wall_ns: AtomicU64,
}

/// Serializable point-in-time view of a [`PerfEntity`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct PerfSnapshot {
    pub(crate) name: &'static str,
    pub(crate) calls: u64,
    pub(crate) success: u64,
    pub(crate) fail: u64,
    pub(crate) wall_ns: u64,
    pub(crate) avg_ns_per_call: u64,
}

/// RAII guard that records elapsed wall time when ended or dropped.
/// Do not hold this guard across `.await`; end it before suspension points.
pub(crate) struct PerfScope<'a> {
    entity: &'a PerfEntity,
    started: Instant,
    ended: bool,
}

impl PerfEntity {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self {
            name,
            calls: AtomicU64::new(0),
            success: AtomicU64::new(0),
            wall_ns: AtomicU64::new(0),
        }
    }

    pub(crate) fn begin(&self) -> PerfScope<'_> {
        PerfScope {
            entity: self,
            started: Instant::now(),
            ended: false,
        }
    }

    pub(crate) fn snapshot(&self) -> PerfSnapshot {
        let calls = self.calls.load(Ordering::Relaxed);
        let success = self.success.load(Ordering::Relaxed);
        let wall_ns = self.wall_ns.load(Ordering::Relaxed);
        PerfSnapshot {
            name: self.name,
            calls,
            success,
            fail: calls.saturating_sub(success),
            wall_ns,
            avg_ns_per_call: if calls == 0 { 0 } else { wall_ns / calls },
        }
    }
}

impl PerfScope<'_> {
    pub(crate) fn end(mut self, success: bool) {
        self.finish(success);
    }

    fn finish(&mut self, success: bool) {
        if self.ended {
            return;
        }
        self.ended = true;
        let elapsed = self.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.entity.calls.fetch_add(1, Ordering::Relaxed);
        if success {
            self.entity.success.fetch_add(1, Ordering::Relaxed);
        }
        self.entity.wall_ns.fetch_add(elapsed, Ordering::Relaxed);
    }
}

impl Drop for PerfScope<'_> {
    fn drop(&mut self) {
        self.finish(false);
    }
}

pub(crate) static PERF_ED25519: PerfEntity = PerfEntity::new("ed25519_verify");
pub(crate) static PERF_STATE_APPLY: PerfEntity = PerfEntity::new("state_apply");
pub(crate) static PERF_CHAIN_SEAL: PerfEntity = PerfEntity::new("chain_seal");
pub(crate) static PERF_POOL_DRAIN: PerfEntity = PerfEntity::new("pool_drain");

pub(crate) static REGISTRY: &[&PerfEntity] = &[
    &PERF_ED25519,
    &PERF_STATE_APPLY,
    &PERF_CHAIN_SEAL,
    &PERF_POOL_DRAIN,
];

#[cfg(test)]
mod tests {
    use super::PerfEntity;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn perf_scope_end_ok() {
        let entity = PerfEntity::new("test_end_ok");
        let scope = entity.begin();
        thread::sleep(Duration::from_micros(1));
        scope.end(true);

        let snap = entity.snapshot();
        assert_eq!(snap.name, "test_end_ok");
        assert_eq!(snap.calls, 1);
        assert_eq!(snap.success, 1);
        assert_eq!(snap.fail, 0);
        assert!(snap.wall_ns > 0);
        assert_eq!(snap.avg_ns_per_call, snap.wall_ns);
    }

    #[test]
    fn perf_scope_drop_fail() {
        let entity = PerfEntity::new("test_drop_fail");
        {
            let _scope = entity.begin();
            thread::sleep(Duration::from_micros(1));
        }

        let snap = entity.snapshot();
        assert_eq!(snap.name, "test_drop_fail");
        assert_eq!(snap.calls, 1);
        assert_eq!(snap.success, 0);
        assert_eq!(snap.fail, 1);
        assert!(snap.wall_ns > 0);
    }

    #[test]
    fn perf_scope_end_no_double() {
        let entity = PerfEntity::new("test_end_no_double");
        {
            let scope = entity.begin();
            thread::sleep(Duration::from_micros(1));
            scope.end(true);
        }

        let snap = entity.snapshot();
        assert_eq!(snap.name, "test_end_no_double");
        assert_eq!(snap.calls, 1);
        assert_eq!(snap.success, 1);
        assert_eq!(snap.fail, 0);
        assert!(snap.wall_ns > 0);
    }
}
