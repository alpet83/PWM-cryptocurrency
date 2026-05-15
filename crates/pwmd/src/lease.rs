//! Lease runtime state machine with pluggable CAS-backed lease storage.

use crate::lease_backend::{
    AcquireRes, LeaseBackend, LeaseGrant, LeaseKeep, LeaseRec, LeaseTake, RenewRes, TakeRes,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    ActiveSealing,
    StandbySyncing,
    SuspectActiveLost,
    FencedStandby,
}

#[derive(Clone, Copy, Debug)]
pub struct LeaseCfg {
    pub ttl_ms: u64,
    pub takeover_ms: u64,
    pub max_tip_lag: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LeaseBackendMode {
    #[default]
    File,
    ProcessLocal,
}

impl Default for LeaseCfg {
    fn default() -> Self {
        Self {
            ttl_ms: 10_000,
            takeover_ms: 8_000,
            max_tip_lag: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LeaseRuntime {
    pub owner_id: String,
    pub term: u64,
    pub expires_at_ms: u64,
    pub last_tip: u64,
    pub fence: u64,
    pub allow_seal: bool,
    pub state: LeaseState,
    pub last_reason: String,
}

impl LeaseRuntime {
    pub fn new(owner_id: String) -> Self {
        Self {
            owner_id,
            term: 0,
            expires_at_ms: 0,
            last_tip: 0,
            fence: 0,
            allow_seal: false,
            state: LeaseState::StandbySyncing,
            last_reason: "lease_not_acquired".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeaseSignal {
    pub owner_id: String,
    pub term: u64,
    pub expires_at_ms: u64,
    pub last_tip: u64,
    pub fence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseEvent {
    Acquire,
    Renew,
    Takeover,
    Loss,
    Reject,
}

#[derive(Clone, Debug)]
pub struct LeaseStep {
    pub allow_seal: bool,
    pub event: Option<LeaseEvent>,
}

#[derive(Default)]
pub struct LeaseStats {
    acquire_ok: AtomicU64,
    renew_ok: AtomicU64,
    loss_total: AtomicU64,
    reject_total: AtomicU64,
    takeover_ok: AtomicU64,
}

impl LeaseStats {
    pub fn on_event(&self, ev: LeaseEvent) {
        match ev {
            LeaseEvent::Acquire => {
                self.acquire_ok.fetch_add(1, Ordering::Relaxed);
            }
            LeaseEvent::Renew => {
                self.renew_ok.fetch_add(1, Ordering::Relaxed);
            }
            LeaseEvent::Takeover => {
                self.takeover_ok.fetch_add(1, Ordering::Relaxed);
            }
            LeaseEvent::Loss => {
                self.loss_total.fetch_add(1, Ordering::Relaxed);
            }
            LeaseEvent::Reject => {
                self.reject_total.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn snapshot(&self) -> LeaseStatsOut {
        LeaseStatsOut {
            acquire_ok: self.acquire_ok.load(Ordering::Relaxed),
            renew_ok: self.renew_ok.load(Ordering::Relaxed),
            loss_total: self.loss_total.load(Ordering::Relaxed),
            reject_total: self.reject_total.load(Ordering::Relaxed),
            takeover_ok: self.takeover_ok.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LeaseStatsOut {
    pub acquire_ok: u64,
    pub renew_ok: u64,
    pub loss_total: u64,
    pub reject_total: u64,
    pub takeover_ok: u64,
}

pub fn step_lease(
    vh: &str,
    node_id: &str,
    now_ms: u64,
    tip_h: u64,
    cfg: LeaseCfg,
    rt: &mut LeaseRuntime,
    backend: &dyn LeaseBackend,
) -> LeaseStep {
    let prev_allow = rt.allow_seal;
    let acq = backend.acquire(LeaseGrant {
        vh,
        owner: node_id,
        now_ms,
        tip_h,
        ttl_ms: cfg.ttl_ms,
    });
    let event = match acq {
        Err(e) => {
            rt.allow_seal = false;
            rt.state = LeaseState::FencedStandby;
            rt.last_reason = format!("lease_backend_error {e}");
            Some(LeaseEvent::Reject)
        }
        Ok(AcquireRes::Acquired(next)) => {
            apply_rt(rt, &next, true);
            rt.state = LeaseState::ActiveSealing;
            rt.last_reason = "lease_acquired_boot".to_string();
            Some(LeaseEvent::Acquire)
        }
        Ok(AcquireRes::Held(rec)) => {
            if rec.owner_id == node_id {
                match backend.renew(LeaseKeep {
                    vh,
                    owner: node_id,
                    exp_term: rec.term,
                    exp_fence: rec.fence,
                    now_ms,
                    tip_h,
                    ttl_ms: cfg.ttl_ms,
                }) {
                    Err(e) => {
                        rt.allow_seal = false;
                        rt.state = LeaseState::FencedStandby;
                        rt.last_reason = format!("lease_backend_error {e}");
                        Some(LeaseEvent::Reject)
                    }
                    Ok(RenewRes::Renewed(next)) => {
                        apply_rt(rt, &next, true);
                        rt.state = LeaseState::ActiveSealing;
                        rt.last_reason = "lease_renewed".to_string();
                        Some(if prev_allow {
                            LeaseEvent::Renew
                        } else {
                            LeaseEvent::Acquire
                        })
                    }
                    Ok(RenewRes::Lost(obs)) => {
                        let why = obs
                            .map(loss_label)
                            .unwrap_or_else(|| "lease_renew_missing".to_string());
                        rt.allow_seal = false;
                        rt.state = LeaseState::StandbySyncing;
                        rt.last_reason = format!("lease_renew_cas_miss {why}");
                        Some(if prev_allow {
                            LeaseEvent::Loss
                        } else {
                            LeaseEvent::Reject
                        })
                    }
                }
            } else {
                step_standby(
                    vh, node_id, now_ms, tip_h, cfg, rt, prev_allow, &rec, backend,
                )
            }
        }
    };
    LeaseStep {
        allow_seal: rt.allow_seal,
        event,
    }
}

fn step_standby(
    vh: &str,
    node_id: &str,
    now_ms: u64,
    tip_h: u64,
    cfg: LeaseCfg,
    rt: &mut LeaseRuntime,
    prev_allow: bool,
    rec: &LeaseRec,
    backend: &dyn LeaseBackend,
) -> Option<LeaseEvent> {
    if now_ms < rec.expiry {
        rt.allow_seal = false;
        rt.state = LeaseState::StandbySyncing;
        rt.last_reason = format!(
            "lease_held_by_peer owner={} term={} expires_at_ms={}",
            rec.owner_id, rec.term, rec.expiry
        );
        return Some(if prev_allow {
            LeaseEvent::Loss
        } else {
            LeaseEvent::Reject
        });
    }
    let take_at = rec.expiry.saturating_add(cfg.takeover_ms);
    if now_ms < take_at {
        rt.allow_seal = false;
        rt.state = LeaseState::SuspectActiveLost;
        rt.last_reason = format!(
            "takeover_wait owner={} takeover_at_ms={}",
            rec.owner_id, take_at
        );
        return Some(if prev_allow {
            LeaseEvent::Loss
        } else {
            LeaseEvent::Reject
        });
    }
    if tip_h.saturating_add(cfg.max_tip_lag) < rec.last_tip {
        rt.allow_seal = false;
        rt.state = LeaseState::StandbySyncing;
        rt.last_reason = format!(
            "takeover_reject_stale_tip local_tip={} required_tip={}",
            tip_h, rec.last_tip
        );
        return Some(if prev_allow {
            LeaseEvent::Loss
        } else {
            LeaseEvent::Reject
        });
    }
    match backend.takeover(LeaseTake {
        vh,
        owner: node_id,
        exp_term: rec.term,
        exp_fence: rec.fence,
        exp_expiry: rec.expiry,
        now_ms,
        tip_h,
        ttl_ms: cfg.ttl_ms,
    }) {
        Err(e) => {
            rt.allow_seal = false;
            rt.state = LeaseState::FencedStandby;
            rt.last_reason = format!("lease_backend_error {e}");
            Some(LeaseEvent::Reject)
        }
        Ok(TakeRes::Taken(next)) => {
            apply_rt(rt, &next, true);
            rt.state = LeaseState::ActiveSealing;
            rt.last_reason = "lease_takeover_committed".to_string();
            Some(LeaseEvent::Takeover)
        }
        Ok(TakeRes::CasMiss(cur)) => {
            rt.allow_seal = false;
            rt.state = LeaseState::StandbySyncing;
            rt.last_reason = format!("lease_takeover_cas_miss {}", loss_label(cur));
            Some(if prev_allow {
                LeaseEvent::Loss
            } else {
                LeaseEvent::Reject
            })
        }
        Ok(TakeRes::Missing) => {
            rt.allow_seal = false;
            rt.state = LeaseState::StandbySyncing;
            rt.last_reason = "lease_takeover_missing".to_string();
            Some(if prev_allow {
                LeaseEvent::Loss
            } else {
                LeaseEvent::Reject
            })
        }
    }
}

fn loss_label(rec: LeaseRec) -> String {
    format!(
        "owner={} term={} fence={} expiry={}",
        rec.owner_id, rec.term, rec.fence, rec.expiry
    )
}

fn apply_rt(rt: &mut LeaseRuntime, rec: &LeaseRec, allow: bool) {
    rt.owner_id = rec.owner_id.clone();
    rt.term = rec.term;
    rt.expires_at_ms = rec.expiry;
    rt.last_tip = rec.last_tip;
    rt.fence = rec.fence;
    rt.allow_seal = allow;
}

#[cfg(test)]
mod tests {
    use super::{step_lease, LeaseCfg, LeaseEvent, LeaseRuntime, LeaseState};
    use crate::lease_backend::{
        FileLeaseBackend, LeaseBackend, LeaseDrop, ProcessLocalLeaseBackend, ReleaseRes,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn step_lease_backend_err_closed() {
        let cfg = LeaseCfg::default();
        let be = crate::lease_backend::ErrLeaseBackend { msg: "boom" };
        let mut rt = LeaseRuntime::new("n1".to_string());
        let step = step_lease("vh-x", "n1", 1_000, 3, cfg, &mut rt, &be);
        assert!(!step.allow_seal);
        assert_eq!(step.event, Some(LeaseEvent::Reject));
        assert_eq!(rt.state, LeaseState::FencedStandby);
        assert!(
            rt.last_reason.starts_with("lease_backend_error "),
            "reason={}",
            rt.last_reason
        );
        assert!(rt.last_reason.contains("boom"), "reason={}", rt.last_reason);
    }

    #[test]
    fn lease_renew_ok_same_owner() {
        let cfg = LeaseCfg::default();
        let be = ProcessLocalLeaseBackend;
        let mut rt = LeaseRuntime::new("n1".to_string());
        let one = step_lease("vh-1", "n1", 1_000, 5, cfg, &mut rt, &be);
        assert!(one.allow_seal);
        assert_eq!(one.event, Some(LeaseEvent::Acquire));
        let two = step_lease("vh-1", "n1", 1_500, 6, cfg, &mut rt, &be);
        assert!(two.allow_seal);
        assert_eq!(two.event, Some(LeaseEvent::Renew));
    }

    #[test]
    fn lease_takeover_after_timeout() {
        let cfg = LeaseCfg {
            ttl_ms: 1_000,
            takeover_ms: 500,
            max_tip_lag: 0,
        };
        let be = ProcessLocalLeaseBackend;
        let mut a = LeaseRuntime::new("a".to_string());
        let mut b = LeaseRuntime::new("b".to_string());
        let _ = step_lease("vh-2", "a", 1_000, 10, cfg, &mut a, &be);
        let blocked = step_lease("vh-2", "b", 1_200, 10, cfg, &mut b, &be);
        assert!(!blocked.allow_seal);
        assert_eq!(b.state, LeaseState::StandbySyncing);
        let takeover = step_lease("vh-2", "b", 2_600, 10, cfg, &mut b, &be);
        assert!(takeover.allow_seal);
        assert_eq!(takeover.event, Some(LeaseEvent::Takeover));
    }

    #[test]
    fn old_active_blocked_without_lease() {
        let cfg = LeaseCfg {
            ttl_ms: 1_000,
            takeover_ms: 500,
            max_tip_lag: 0,
        };
        let be = ProcessLocalLeaseBackend;
        let mut a = LeaseRuntime::new("a".to_string());
        let mut b = LeaseRuntime::new("b".to_string());
        let _ = step_lease("vh-3", "a", 1_000, 5, cfg, &mut a, &be);
        let _ = step_lease("vh-3", "b", 2_600, 5, cfg, &mut b, &be);
        let blocked = step_lease("vh-3", "a", 2_650, 5, cfg, &mut a, &be);
        assert!(!blocked.allow_seal);
        assert_eq!(blocked.event, Some(LeaseEvent::Loss));
        assert_eq!(a.state, LeaseState::StandbySyncing);
    }

    #[test]
    fn lease_release_cas_ok() {
        let cfg = LeaseCfg::default();
        let be = ProcessLocalLeaseBackend;
        let mut rt = LeaseRuntime::new("n9".to_string());
        let _ = step_lease("vh-rel", "n9", 1_000, 4, cfg, &mut rt, &be);
        let got = be
            .release(LeaseDrop {
                vh: "vh-rel",
                owner: &rt.owner_id,
                exp_term: rt.term,
                exp_fence: rt.fence,
            })
            .expect("release");
        assert_eq!(got, ReleaseRes::Released);
    }

    #[test]
    fn file_two_node_takeover_sim() {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwmd-lease-two-node-{ns}"));
        let be = FileLeaseBackend::open(dir.clone()).expect("file backend");
        let cfg = LeaseCfg {
            ttl_ms: 1_000,
            takeover_ms: 500,
            max_tip_lag: 0,
        };
        let mut a = LeaseRuntime::new("a".to_string());
        let mut b = LeaseRuntime::new("b".to_string());
        let a_boot = step_lease("vh-sim", "a", 1_000, 10, cfg, &mut a, &be);
        assert!(a_boot.allow_seal);
        let b_blocked = step_lease("vh-sim", "b", 1_100, 10, cfg, &mut b, &be);
        assert!(!b_blocked.allow_seal);
        let b_wait = step_lease("vh-sim", "b", 2_200, 10, cfg, &mut b, &be);
        assert!(!b_wait.allow_seal);
        assert_eq!(b.state, LeaseState::SuspectActiveLost);
        let b_take = step_lease("vh-sim", "b", 2_600, 10, cfg, &mut b, &be);
        assert!(b_take.allow_seal);
        assert_eq!(b_take.event, Some(LeaseEvent::Takeover));
        let a_old = step_lease("vh-sim", "a", 2_650, 10, cfg, &mut a, &be);
        assert!(!a_old.allow_seal);
        let _ = fs::remove_dir_all(dir);
    }
}
