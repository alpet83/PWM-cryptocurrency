//! Lease backend trait with process-local and file CAS implementations.

use crate::lease::LeaseBackendMode;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseRec {
    pub owner_id: String,
    pub validator_identity_hash: String,
    pub term: u64,
    pub fence: u64,
    pub expiry: u64,
    pub last_tip: u64,
    pub updated_at: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct LeaseGrant<'a> {
    pub vh: &'a str,
    pub owner: &'a str,
    pub now_ms: u64,
    pub tip_h: u64,
    pub ttl_ms: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct LeaseKeep<'a> {
    pub vh: &'a str,
    pub owner: &'a str,
    pub exp_term: u64,
    pub exp_fence: u64,
    pub now_ms: u64,
    pub tip_h: u64,
    pub ttl_ms: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct LeaseTake<'a> {
    pub vh: &'a str,
    pub owner: &'a str,
    pub exp_term: u64,
    pub exp_fence: u64,
    pub exp_expiry: u64,
    pub now_ms: u64,
    pub tip_h: u64,
    pub ttl_ms: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct LeaseDrop<'a> {
    pub vh: &'a str,
    pub owner: &'a str,
    pub exp_term: u64,
    pub exp_fence: u64,
}

#[derive(Clone, Debug)]
pub enum AcquireRes {
    Acquired(LeaseRec),
    Held(LeaseRec),
}

#[derive(Clone, Debug)]
pub enum RenewRes {
    Renewed(LeaseRec),
    Lost(Option<LeaseRec>),
}

#[derive(Clone, Debug)]
pub enum TakeRes {
    Taken(LeaseRec),
    CasMiss(LeaseRec),
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseRes {
    Released,
    NotOwner,
}

#[allow(dead_code)]
pub trait LeaseBackend: Send + Sync {
    fn mode(&self) -> LeaseBackendMode;
    fn lease_path(&self) -> Option<PathBuf>;
    fn acquire(&self, req: LeaseGrant<'_>) -> Result<AcquireRes, String>;
    fn renew(&self, req: LeaseKeep<'_>) -> Result<RenewRes, String>;
    fn release(&self, req: LeaseDrop<'_>) -> Result<ReleaseRes, String>;
    fn takeover(&self, req: LeaseTake<'_>) -> Result<TakeRes, String>;
}

fn mk_rec(
    vh: &str,
    owner: &str,
    now_ms: u64,
    tip_h: u64,
    ttl_ms: u64,
    term: u64,
    fence: u64,
) -> LeaseRec {
    LeaseRec {
        owner_id: owner.to_string(),
        validator_identity_hash: vh.to_string(),
        term,
        fence,
        expiry: now_ms.saturating_add(ttl_ms),
        last_tip: tip_h,
        updated_at: now_ms,
    }
}

#[derive(Default)]
pub struct ProcessLocalLeaseBackend;

#[derive(Clone, Debug)]
struct ProcStore {
    by_key: HashMap<String, LeaseRec>,
}

static PROC_STORE: OnceLock<Mutex<ProcStore>> = OnceLock::new();

fn proc_store() -> &'static Mutex<ProcStore> {
    PROC_STORE.get_or_init(|| {
        Mutex::new(ProcStore {
            by_key: HashMap::new(),
        })
    })
}

impl LeaseBackend for ProcessLocalLeaseBackend {
    fn mode(&self) -> LeaseBackendMode {
        LeaseBackendMode::ProcessLocal
    }

    fn lease_path(&self) -> Option<PathBuf> {
        None
    }

    fn acquire(&self, req: LeaseGrant<'_>) -> Result<AcquireRes, String> {
        let mut guard = proc_store()
            .lock()
            .map_err(|_| "lease_store_poisoned".to_string())?;
        match guard.by_key.get(req.vh).cloned() {
            None => {
                let rec = mk_rec(req.vh, req.owner, req.now_ms, req.tip_h, req.ttl_ms, 1, 1);
                guard.by_key.insert(req.vh.to_string(), rec.clone());
                Ok(AcquireRes::Acquired(rec))
            }
            Some(rec) => Ok(AcquireRes::Held(rec)),
        }
    }

    fn renew(&self, req: LeaseKeep<'_>) -> Result<RenewRes, String> {
        let mut guard = proc_store()
            .lock()
            .map_err(|_| "lease_store_poisoned".to_string())?;
        let Some(cur) = guard.by_key.get(req.vh).cloned() else {
            return Ok(RenewRes::Lost(None));
        };
        if cur.owner_id != req.owner
            || cur.term != req.exp_term
            || cur.fence != req.exp_fence
            || req.now_ms > cur.expiry
        {
            return Ok(RenewRes::Lost(Some(cur)));
        }
        let mut next = cur.clone();
        next.expiry = req.now_ms.saturating_add(req.ttl_ms);
        next.last_tip = req.tip_h;
        next.updated_at = req.now_ms;
        guard.by_key.insert(req.vh.to_string(), next.clone());
        Ok(RenewRes::Renewed(next))
    }

    fn release(&self, req: LeaseDrop<'_>) -> Result<ReleaseRes, String> {
        let mut guard = proc_store()
            .lock()
            .map_err(|_| "lease_store_poisoned".to_string())?;
        let Some(cur) = guard.by_key.get(req.vh) else {
            return Ok(ReleaseRes::NotOwner);
        };
        if cur.owner_id != req.owner || cur.term != req.exp_term || cur.fence != req.exp_fence {
            return Ok(ReleaseRes::NotOwner);
        }
        guard.by_key.remove(req.vh);
        Ok(ReleaseRes::Released)
    }

    fn takeover(&self, req: LeaseTake<'_>) -> Result<TakeRes, String> {
        let mut guard = proc_store()
            .lock()
            .map_err(|_| "lease_store_poisoned".to_string())?;
        let Some(cur) = guard.by_key.get(req.vh).cloned() else {
            return Ok(TakeRes::Missing);
        };
        if cur.term != req.exp_term || cur.fence != req.exp_fence || cur.expiry != req.exp_expiry {
            return Ok(TakeRes::CasMiss(cur));
        }
        let rec = mk_rec(
            req.vh,
            req.owner,
            req.now_ms,
            req.tip_h,
            req.ttl_ms,
            cur.term.saturating_add(1),
            cur.fence.saturating_add(1),
        );
        guard.by_key.insert(req.vh.to_string(), rec.clone());
        Ok(TakeRes::Taken(rec))
    }
}

pub struct FileLeaseBackend {
    root: PathBuf,
    nonce: AtomicU64,
}

impl FileLeaseBackend {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|e| {
            format!(
                "lease_backend_dir_create_failed path={}: {e}",
                root.display()
            )
        })?;
        Ok(Self {
            root,
            nonce: AtomicU64::new(1),
        })
    }

    fn lease_path_for(&self, vh: &str) -> PathBuf {
        self.root.join(format!("{vh}.lease.json"))
    }

    fn lock_path_for(&self, vh: &str) -> PathBuf {
        self.root.join(format!("{vh}.lease.lock"))
    }

    fn temp_path_for(&self, vh: &str) -> PathBuf {
        let pid = std::process::id();
        let ns = now_ms();
        let nonce = self.nonce.fetch_add(1, Ordering::Relaxed);
        self.root.join(format!("{vh}.lease.tmp-{pid}-{ns}-{nonce}"))
    }

    fn with_key_lock<T, F>(&self, vh: &str, f: F) -> Result<T, String>
    where
        F: FnOnce(&Path) -> Result<T, String>,
    {
        let lock_path = self.lock_path_for(vh);
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|e| {
                format!(
                    "lease_backend_lock_open_failed path={}: {e}",
                    lock_path.display()
                )
            })?;
        lock.lock_exclusive().map_err(|e| {
            format!(
                "lease_backend_lock_acquire_failed path={}: {e}",
                lock_path.display()
            )
        })?;
        let lease_path = self.lease_path_for(vh);
        let out = f(&lease_path);
        let _ = fs2::FileExt::unlock(&lock);
        out
    }

    fn read_rec(path: &Path) -> Result<Option<LeaseRec>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let mut buf = Vec::new();
        File::open(path)
            .and_then(|mut f| {
                f.read_to_end(&mut buf)?;
                Ok(())
            })
            .map_err(|e| format!("lease_backend_read_failed path={}: {e}", path.display()))?;
        let rec: LeaseRec = serde_json::from_slice(&buf)
            .map_err(|e| format!("lease_backend_corrupt_record path={}: {e}", path.display()))?;
        Ok(Some(rec))
    }

    fn write_rec(&self, path: &Path, rec: &LeaseRec) -> Result<(), String> {
        let tmp_path = self.temp_path_for(&rec.validator_identity_hash);
        let payload = serde_json::to_vec_pretty(rec)
            .map_err(|e| format!("lease_backend_serialize_failed: {e}"))?;
        {
            let mut tmp = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)
                .map_err(|e| {
                    format!(
                        "lease_backend_tmp_open_failed path={}: {e}",
                        tmp_path.display()
                    )
                })?;
            tmp.write_all(&payload).map_err(|e| {
                format!(
                    "lease_backend_tmp_write_failed path={}: {e}",
                    tmp_path.display()
                )
            })?;
            tmp.sync_all().map_err(|e| {
                format!(
                    "lease_backend_tmp_sync_failed path={}: {e}",
                    tmp_path.display()
                )
            })?;
        }
        if path.exists() {
            fs::remove_file(path).map_err(|e| {
                format!(
                    "lease_backend_target_remove_failed path={}: {e}",
                    path.display()
                )
            })?;
        }
        fs::rename(&tmp_path, path).map_err(|e| {
            format!(
                "lease_backend_atomic_rename_failed src={} dst={}: {e}",
                tmp_path.display(),
                path.display()
            )
        })?;
        Ok(())
    }

    fn del_rec(path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        fs::remove_file(path)
            .map_err(|e| format!("lease_backend_delete_failed path={}: {e}", path.display()))
    }

    fn read_checked(path: &Path, vh: &str) -> Result<Option<LeaseRec>, String> {
        let rec = Self::read_rec(path)?;
        if let Some(r) = rec.as_ref() {
            if r.validator_identity_hash != vh {
                return Err(format!(
                    "lease_backend_validator_mismatch path={} expected={} actual={}",
                    path.display(),
                    vh,
                    r.validator_identity_hash
                ));
            }
        }
        Ok(rec)
    }
}

impl LeaseBackend for FileLeaseBackend {
    fn mode(&self) -> LeaseBackendMode {
        LeaseBackendMode::File
    }

    fn lease_path(&self) -> Option<PathBuf> {
        Some(self.root.clone())
    }

    fn acquire(&self, req: LeaseGrant<'_>) -> Result<AcquireRes, String> {
        self.with_key_lock(req.vh, |path| {
            let cur = Self::read_checked(path, req.vh)?;
            match cur {
                None => {
                    let rec = mk_rec(req.vh, req.owner, req.now_ms, req.tip_h, req.ttl_ms, 1, 1);
                    self.write_rec(path, &rec)?;
                    Ok(AcquireRes::Acquired(rec))
                }
                Some(rec) => Ok(AcquireRes::Held(rec)),
            }
        })
    }

    fn renew(&self, req: LeaseKeep<'_>) -> Result<RenewRes, String> {
        self.with_key_lock(req.vh, |path| {
            let Some(cur) = Self::read_checked(path, req.vh)? else {
                return Ok(RenewRes::Lost(None));
            };
            if cur.owner_id != req.owner
                || cur.term != req.exp_term
                || cur.fence != req.exp_fence
                || req.now_ms > cur.expiry
            {
                return Ok(RenewRes::Lost(Some(cur)));
            }
            let mut next = cur.clone();
            next.expiry = req.now_ms.saturating_add(req.ttl_ms);
            next.last_tip = req.tip_h;
            next.updated_at = req.now_ms;
            self.write_rec(path, &next)?;
            Ok(RenewRes::Renewed(next))
        })
    }

    fn release(&self, req: LeaseDrop<'_>) -> Result<ReleaseRes, String> {
        self.with_key_lock(req.vh, |path| {
            let Some(cur) = Self::read_checked(path, req.vh)? else {
                return Ok(ReleaseRes::NotOwner);
            };
            if cur.owner_id != req.owner || cur.term != req.exp_term || cur.fence != req.exp_fence {
                return Ok(ReleaseRes::NotOwner);
            }
            Self::del_rec(path)?;
            Ok(ReleaseRes::Released)
        })
    }

    fn takeover(&self, req: LeaseTake<'_>) -> Result<TakeRes, String> {
        self.with_key_lock(req.vh, |path| {
            let Some(cur) = Self::read_checked(path, req.vh)? else {
                return Ok(TakeRes::Missing);
            };
            if cur.term != req.exp_term
                || cur.fence != req.exp_fence
                || cur.expiry != req.exp_expiry
            {
                return Ok(TakeRes::CasMiss(cur));
            }
            let rec = mk_rec(
                req.vh,
                req.owner,
                req.now_ms,
                req.tip_h,
                req.ttl_ms,
                cur.term.saturating_add(1),
                cur.fence.saturating_add(1),
            );
            self.write_rec(path, &rec)?;
            Ok(TakeRes::Taken(rec))
        })
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis() as u64)
        .unwrap_or(0)
}

/// Deterministic failing backend for unit tests (fail-closed gate coverage).
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrLeaseBackend {
    pub msg: &'static str,
}

#[cfg(test)]
impl LeaseBackend for ErrLeaseBackend {
    fn mode(&self) -> LeaseBackendMode {
        LeaseBackendMode::ProcessLocal
    }

    fn lease_path(&self) -> Option<PathBuf> {
        None
    }

    fn acquire(&self, _req: LeaseGrant<'_>) -> Result<AcquireRes, String> {
        Err(self.msg.to_string())
    }

    fn renew(&self, _req: LeaseKeep<'_>) -> Result<RenewRes, String> {
        Err(self.msg.to_string())
    }

    fn release(&self, _req: LeaseDrop<'_>) -> Result<ReleaseRes, String> {
        Err(self.msg.to_string())
    }

    fn takeover(&self, _req: LeaseTake<'_>) -> Result<TakeRes, String> {
        Err(self.msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_tmp_dir(tag: &str) -> PathBuf {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("pwmd-lease-{tag}-{ns}"))
    }

    #[test]
    fn file_acq_then_renew_ok() {
        let dir = mk_tmp_dir("renew");
        let be = FileLeaseBackend::open(dir.clone()).expect("backend");
        let got = be
            .acquire(LeaseGrant {
                vh: "vh-1",
                owner: "n1",
                now_ms: 1_000,
                tip_h: 10,
                ttl_ms: 500,
            })
            .expect("acquire");
        let rec = match got {
            AcquireRes::Acquired(v) => v,
            _ => panic!("must acquire"),
        };
        let renewed = be
            .renew(LeaseKeep {
                vh: "vh-1",
                owner: "n1",
                exp_term: rec.term,
                exp_fence: rec.fence,
                now_ms: 1_200,
                tip_h: 11,
                ttl_ms: 500,
            })
            .expect("renew");
        match renewed {
            RenewRes::Renewed(v) => {
                assert_eq!(v.term, rec.term);
                assert_eq!(v.fence, rec.fence);
                assert_eq!(v.last_tip, 11);
            }
            _ => panic!("must renew"),
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_takeover_cas_gate() {
        let dir = mk_tmp_dir("take");
        let be = FileLeaseBackend::open(dir.clone()).expect("backend");
        let held = be
            .acquire(LeaseGrant {
                vh: "vh-2",
                owner: "a",
                now_ms: 1_000,
                tip_h: 3,
                ttl_ms: 200,
            })
            .expect("acquire");
        let rec = match held {
            AcquireRes::Acquired(v) => v,
            _ => panic!("must acquire"),
        };
        let miss = be
            .takeover(LeaseTake {
                vh: "vh-2",
                owner: "b",
                exp_term: rec.term,
                exp_fence: rec.fence,
                exp_expiry: rec.expiry.saturating_sub(1),
                now_ms: 1_300,
                tip_h: 3,
                ttl_ms: 200,
            })
            .expect("take miss");
        assert!(matches!(miss, TakeRes::CasMiss(_)));
        let ok = be
            .takeover(LeaseTake {
                vh: "vh-2",
                owner: "b",
                exp_term: rec.term,
                exp_fence: rec.fence,
                exp_expiry: rec.expiry,
                now_ms: 1_300,
                tip_h: 4,
                ttl_ms: 200,
            })
            .expect("take ok");
        match ok {
            TakeRes::Taken(v) => {
                assert_eq!(v.owner_id, "b");
                assert_eq!(v.term, rec.term + 1);
                assert_eq!(v.fence, rec.fence + 1);
            }
            _ => panic!("must takeover"),
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_release_cas_gate() {
        let dir = mk_tmp_dir("drop");
        let be = FileLeaseBackend::open(dir.clone()).expect("backend");
        let held = be
            .acquire(LeaseGrant {
                vh: "vh-3",
                owner: "a",
                now_ms: 1_000,
                tip_h: 1,
                ttl_ms: 100,
            })
            .expect("acquire");
        let rec = match held {
            AcquireRes::Acquired(v) => v,
            _ => panic!("must acquire"),
        };
        let not_owner = be
            .release(LeaseDrop {
                vh: "vh-3",
                owner: "b",
                exp_term: rec.term,
                exp_fence: rec.fence,
            })
            .expect("release");
        assert_eq!(not_owner, ReleaseRes::NotOwner);
        let released = be
            .release(LeaseDrop {
                vh: "vh-3",
                owner: "a",
                exp_term: rec.term,
                exp_fence: rec.fence,
            })
            .expect("release");
        assert_eq!(released, ReleaseRes::Released);
        let _ = fs::remove_dir_all(dir);
    }
}
