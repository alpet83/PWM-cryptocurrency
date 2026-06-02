//! Nonblocking per-block RFC16 timing JSONL capture with deferred in-memory queue.

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const DEF_PATH: &str = "tmp/cy-lab-block-timing.jsonl";
const PEND_MAX_RECORDS: usize = 1500;

#[derive(Clone, Debug)]
pub(crate) struct BlockTimingCfg {
    pub on: bool,
    pub path: PathBuf,
    pub pend_path: PathBuf,
    pub lock_path: PathBuf,
    pub cluster_id: String,
    pub prop_id: String,
    pub pwmd_marker: String,
}

impl Default for BlockTimingCfg {
    fn default() -> Self {
        let path = PathBuf::from(DEF_PATH);
        Self {
            on: false,
            pend_path: path.with_extension("pending.json"),
            lock_path: path.with_extension("lock"),
            path,
            cluster_id: String::new(),
            prop_id: String::new(),
            pwmd_marker: format!("pwmd/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

impl BlockTimingCfg {
    pub(crate) fn mk_new(
        on: bool,
        path: PathBuf,
        cluster_id: String,
        prop_id: String,
        pwmd_marker: String,
    ) -> Self {
        Self {
            on,
            pend_path: path.with_extension("pending.json"),
            lock_path: path.with_extension("lock"),
            path,
            cluster_id,
            prop_id,
            pwmd_marker,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct T0Ctx {
    pub h: u64,
    pub r: u32,
    pub t_ms: f64,
    pub grid_ms: u64,
    pub nom_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SendCtx {
    pub h: u64,
    pub r: u32,
    pub t_ms: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct AttCtx {
    pub h: u64,
    pub r: u32,
    pub t_ms: f64,
    pub att_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcCtx {
    pub h: u64,
    pub r: u32,
    pub start_ms: f64,
    pub proc_ms: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct SealCtx {
    pub h: u64,
    pub r: u32,
    pub seal_ms: f64,
    pub pending_ticks: u64,
    pub gate_recheck: bool,
    pub autosnap: bool,
    pub supp_strike: bool,
    pub attest_to: bool,
    pub nom_ms: u64,
    pub grid_ms: u64,
    pub profile_json: String,
    pub wall_total_ms: f64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProfileTime {
    start_ms: Option<u64>,
    start_at: Option<std::time::Instant>,
    checkpoints_abs_ms: BTreeMap<String, u64>,
    checkpoints_rel_ms: BTreeMap<String, f64>,
}

impl ProfileTime {
    pub(crate) fn start(&mut self, timestamp_ms: Option<u64>) {
        let ts = timestamp_ms.unwrap_or_else(|| crate::current_time_ms().unwrap_or(0));
        self.start_ms = Some(ts);
        self.start_at = Some(std::time::Instant::now());
        self.checkpoints_abs_ms.clear();
        self.checkpoints_rel_ms.clear();
    }

    pub(crate) fn checkpoint(&mut self, name: &str) {
        let ts = crate::current_time_ms().unwrap_or(0);
        self.checkpoint_at(name, ts);
    }

    pub(crate) fn checkpoint_at(&mut self, name: &str, timestamp_ms: u64) {
        if self.start_ms.is_none() {
            self.start_ms = Some(timestamp_ms);
        }
        self.checkpoints_abs_ms
            .insert(name.to_string(), timestamp_ms);
        let rel_ms = if let Some(base) = self.start_ms {
            timestamp_ms.saturating_sub(base) as f64
        } else {
            self.start_at
                .as_ref()
                .map(|started_at| started_at.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(0.0)
        };
        self.checkpoints_rel_ms.insert(name.to_string(), rel_ms);
    }

    #[allow(dead_code)]
    pub(crate) fn json_stats(&self, input: &str) -> String {
        self.json_stats_with_precision(input, 0)
    }

    pub(crate) fn json_stats_with_precision(&self, input: &str, ms_digits: u32) -> String {
        let mut root = if input.trim().is_empty() {
            Map::new()
        } else {
            match serde_json::from_str::<Value>(input) {
                Ok(Value::Object(map)) => map,
                _ => Map::new(),
            }
        };
        let start_ms = self.start_ms.unwrap_or(0);
        let mut checkpoints_abs_ms = Map::new();
        let mut checkpoints_rel_ms = Map::new();
        for (name, ts) in &self.checkpoints_abs_ms {
            checkpoints_abs_ms.insert(name.clone(), Value::from(*ts));
            let rel_ms = self
                .checkpoints_rel_ms
                .get(name)
                .copied()
                .unwrap_or_else(|| ts.saturating_sub(start_ms) as f64);
            checkpoints_rel_ms.insert(name.clone(), Value::from(round_ms(rel_ms, ms_digits)));
        }
        root.insert("start_ms".to_string(), Value::from(start_ms));
        root.insert(
            "checkpoints_abs_ms".to_string(),
            Value::Object(checkpoints_abs_ms),
        );
        root.insert(
            "checkpoints_rel_ms".to_string(),
            Value::Object(checkpoints_rel_ms),
        );
        let wall_total_ms = self
            .start_at
            .as_ref()
            .map(|started_at| round_ms(started_at.elapsed().as_secs_f64() * 1000.0, ms_digits))
            .unwrap_or(0.0);
        root.insert("wall_total_ms".to_string(), Value::from(wall_total_ms));
        Value::Object(root).to_string()
    }
}

fn round_ms(value: f64, digits: u32) -> f64 {
    let factor = 10f64.powi(digits as i32);
    (value * factor).round() / factor
}

pub(crate) fn now_ms_f64() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as f64)
        .unwrap_or(0.0);
    micros / 1000.0
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PendRec {
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    t0_ms: Option<f64>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    prop_send_ms: Option<f64>,
    prop_send_n: u64,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    att_rx_ms: Option<f64>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    att_proc_start_ms: Option<f64>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    att_proc_ms: Option<f64>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    att_wire_ms: Option<f64>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    prop_att_ms: Option<f64>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    gate_ok_ms: Option<f64>,
    att_id: Option<String>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    nom_ms: Option<f64>,
    #[serde(
        serialize_with = "ser_opt_f64_ms2",
        deserialize_with = "de_opt_f64_ms2"
    )]
    grid_ms: Option<f64>,
}

fn ser_opt_f64_ms2<S>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match value {
        Some(v) => serializer.serialize_some(&format!("{:.2}", *v)),
        None => serializer.serialize_none(),
    }
}

fn de_opt_f64_ms2<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<Value>::deserialize(deserializer)?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    match raw {
        Value::Number(n) => {
            if let Some(v) = n.as_f64() {
                if v.is_finite() && v >= 0.0 {
                    Ok(Some(v))
                } else {
                    Err(serde::de::Error::custom(
                        "invalid non-finite or negative ms value",
                    ))
                }
            } else {
                Err(serde::de::Error::custom("invalid ms numeric value"))
            }
        }
        Value::String(s) => {
            let trimmed = s.trim();
            if let Ok(v) = trimmed.parse::<f64>() {
                if v.is_finite() && v >= 0.0 {
                    return Ok(Some(v));
                }
            }
            Err(serde::de::Error::custom("invalid ms string value"))
        }
        _ => Err(serde::de::Error::custom(
            "invalid ms value type, expected number or string",
        )),
    }
}

#[derive(Clone, Debug)]
enum OpKind {
    T0,
    Send,
    AttRx,
    AttProc,
    AttWire,
    AttOk,
    GateOk,
    Seal,
}

#[derive(Clone, Debug)]
struct DefOp {
    kind: OpKind,
    key: String,
    h: u64,
    r: u32,
    t_ms: Option<f64>,
    proc_ms: Option<f64>,
    att_id: Option<String>,
    pending_ticks: u64,
    gate_recheck: bool,
    autosnap: bool,
    supp_strike: bool,
    attest_to: bool,
    nom_ms: u64,
    grid_ms: u64,
    profile_json: Option<String>,
    wall_total_ms: f64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DefQ {
    ops: Vec<DefOp>,
    pub lock_busy_skips: u64,
    pub flushed_ops: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct BlockTiming {
    pub cfg: BlockTimingCfg,
    pub q: Arc<Mutex<DefQ>>,
}

static BT_Q_REG: OnceLock<Mutex<HashMap<String, Arc<Mutex<DefQ>>>>> = OnceLock::new();

#[derive(Clone, Debug, Serialize)]
struct DMs {
    prop_round_open: Option<u64>,
    prop_first_wire_send: Option<u64>,
    prop_wire_resend_total: u64,
    att_rx_propose: Option<u64>,
    att_proc_start: Option<u64>,
    att_proc: Option<u64>,
    att_wire_send: Option<u64>,
    prop_rx_attest: Option<u64>,
    prop_gate_ready: Option<u64>,
    prop_seal_commit: Option<u64>,
    wall_total: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
struct RowOut {
    schema_v: u8,
    cluster_id: String,
    height: u64,
    round: u32,
    sealed_h: u64,
    proposer_instance_id: String,
    attester_instance_id: Option<String>,
    t0_ms: u64,
    d_ms: DMs,
    pending_ticks_at_seal: u64,
    gate_recheck_used: bool,
    autosnapshot_checkpoint: bool,
    suppress_strike: bool,
    attest_timeout: bool,
    nominal_seal_ms: u64,
    grid_deadline_ms: u64,
    seal_slip_ms: i64,
    profile: Value,
    pwmd_marker: String,
}

impl BlockTiming {
    pub(crate) fn mk_new(cfg: BlockTimingCfg) -> Self {
        Self {
            cfg,
            q: Arc::new(Mutex::new(DefQ::default())),
        }
    }

    pub(crate) fn note_t0(&self, ctx: T0Ctx) {
        self.push(DefOp {
            kind: OpKind::T0,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.t_ms),
            proc_ms: None,
            att_id: None,
            pending_ticks: 0,
            gate_recheck: false,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: ctx.nom_ms,
            grid_ms: ctx.grid_ms,
            profile_json: None,
            wall_total_ms: 0.0,
        });
    }

    pub(crate) fn note_send(&self, ctx: SendCtx) {
        self.push(DefOp {
            kind: OpKind::Send,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.t_ms),
            proc_ms: None,
            att_id: None,
            pending_ticks: 0,
            gate_recheck: false,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: 0,
            grid_ms: 0,
            profile_json: None,
            wall_total_ms: 0.0,
        });
    }

    pub(crate) fn note_att_rx(&self, ctx: AttCtx) {
        self.push(DefOp {
            kind: OpKind::AttRx,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.t_ms),
            proc_ms: None,
            att_id: Some(ctx.att_id),
            pending_ticks: 0,
            gate_recheck: false,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: 0,
            grid_ms: 0,
            profile_json: None,
            wall_total_ms: 0.0,
        });
    }

    pub(crate) fn note_att_proc(&self, ctx: ProcCtx) {
        self.push(DefOp {
            kind: OpKind::AttProc,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.start_ms),
            proc_ms: Some(ctx.proc_ms),
            att_id: None,
            pending_ticks: 0,
            gate_recheck: false,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: 0,
            grid_ms: 0,
            profile_json: None,
            wall_total_ms: 0.0,
        });
    }

    pub(crate) fn note_att_wire(&self, ctx: AttCtx) {
        self.push(DefOp {
            kind: OpKind::AttWire,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.t_ms),
            proc_ms: None,
            att_id: Some(ctx.att_id),
            pending_ticks: 0,
            gate_recheck: false,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: 0,
            grid_ms: 0,
            profile_json: None,
            wall_total_ms: 0.0,
        });
    }

    pub(crate) fn note_att_ok(&self, ctx: AttCtx) {
        self.push(DefOp {
            kind: OpKind::AttOk,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.t_ms),
            proc_ms: None,
            att_id: Some(ctx.att_id),
            pending_ticks: 0,
            gate_recheck: false,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: 0,
            grid_ms: 0,
            profile_json: None,
            wall_total_ms: 0.0,
        });
    }

    pub(crate) fn note_gate_ok(&self, ctx: SendCtx) {
        self.push(DefOp {
            kind: OpKind::GateOk,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.t_ms),
            proc_ms: None,
            att_id: None,
            pending_ticks: 0,
            gate_recheck: false,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: 0,
            grid_ms: 0,
            profile_json: None,
            wall_total_ms: 0.0,
        });
    }

    pub(crate) fn note_seal(&self, ctx: SealCtx) {
        self.push(DefOp {
            kind: OpKind::Seal,
            key: key_of(ctx.h, ctx.r),
            h: ctx.h,
            r: ctx.r,
            t_ms: Some(ctx.seal_ms),
            proc_ms: None,
            att_id: None,
            pending_ticks: ctx.pending_ticks,
            gate_recheck: ctx.gate_recheck,
            autosnap: ctx.autosnap,
            supp_strike: ctx.supp_strike,
            attest_to: ctx.attest_to,
            nom_ms: ctx.nom_ms,
            grid_ms: ctx.grid_ms,
            profile_json: Some(ctx.profile_json),
            wall_total_ms: ctx.wall_total_ms,
        });
        let _ = self.try_flush_once();
    }

    pub(crate) fn try_flush_once(&self) -> Result<(), String> {
        if !self.cfg.on {
            return Ok(());
        }
        ensure_parent(&self.cfg.path)?;
        ensure_parent(&self.cfg.pend_path)?;
        ensure_parent(&self.cfg.lock_path)?;

        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&self.cfg.lock_path)
            .map_err(|e| {
                format!(
                    "block_timing lock open failed path={}: {e}",
                    self.cfg.lock_path.display()
                )
            })?;

        if let Err(e) = lock.try_lock_exclusive() {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                if let Ok(mut q) = self.q.lock() {
                    q.lock_busy_skips = q.lock_busy_skips.saturating_add(1);
                }
                return Ok(());
            }
            return Err(format!(
                "block_timing lock acquire failed path={}: {e}",
                self.cfg.lock_path.display()
            ));
        }

        let ops = {
            let mut q = self
                .q
                .lock()
                .map_err(|_| "block_timing queue mutex poisoned".to_string())?;
            if q.ops.is_empty() {
                let _ = fs2::FileExt::unlock(&lock);
                return Ok(());
            }
            std::mem::take(&mut q.ops)
        };

        let out = (|| {
            let mut map = read_pend(&self.cfg.pend_path)?;
            for op in ops.iter() {
                let rec = map.entry(op.key.clone()).or_default();
                apply_op(rec, op);
                if matches!(op.kind, OpKind::Seal) {
                    let rec = map.remove(&op.key).unwrap_or_default();
                    let row = mk_row(&self.cfg, &rec, op);
                    append_jsonl(&self.cfg, &row)?;
                }
            }
            write_pend(&self.cfg.pend_path, &mut map)?;
            Ok(ops.len() as u64)
        })();
        let _ = fs2::FileExt::unlock(&lock);

        match out {
            Ok(n) => {
                if let Ok(mut q) = self.q.lock() {
                    q.flushed_ops = q.flushed_ops.saturating_add(n);
                }
                Ok(())
            }
            Err(e) => {
                if let Ok(mut q) = self.q.lock() {
                    // Requeue ops on I/O failure so sealed row is not lost.
                    q.ops.extend(ops);
                }
                Err(e)
            }
        }
    }

    pub(crate) fn queue_depth(&self) -> u64 {
        self.q.lock().map(|q| q.ops.len() as u64).unwrap_or(0)
    }

    fn push(&self, op: DefOp) {
        if !self.cfg.on {
            return;
        }
        if let Ok(mut q) = self.q.lock() {
            q.ops.push(op);
        }
    }
}

fn q_for_cfg(cfg: &BlockTimingCfg) -> Arc<Mutex<DefQ>> {
    let key = cfg.path.to_string_lossy().to_string();
    let reg = BT_Q_REG.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = reg.lock() {
        return map
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(DefQ::default())))
            .clone();
    }
    Arc::new(Mutex::new(DefQ::default()))
}

fn from_cfg(cfg: &BlockTimingCfg) -> BlockTiming {
    BlockTiming {
        cfg: cfg.clone(),
        q: q_for_cfg(cfg),
    }
}

pub(crate) fn note_t0(cfg: &BlockTimingCfg, ctx: T0Ctx) {
    from_cfg(cfg).note_t0(ctx);
}

pub(crate) fn note_send(cfg: &BlockTimingCfg, ctx: SendCtx) {
    from_cfg(cfg).note_send(ctx);
}

pub(crate) fn note_att_rx(cfg: &BlockTimingCfg, ctx: AttCtx) {
    from_cfg(cfg).note_att_rx(ctx);
}

pub(crate) fn note_att_proc(cfg: &BlockTimingCfg, ctx: ProcCtx) {
    from_cfg(cfg).note_att_proc(ctx);
}

pub(crate) fn note_att_wire(cfg: &BlockTimingCfg, ctx: AttCtx) {
    from_cfg(cfg).note_att_wire(ctx);
}

pub(crate) fn note_att_ok(cfg: &BlockTimingCfg, ctx: AttCtx) {
    from_cfg(cfg).note_att_ok(ctx);
}

pub(crate) fn note_gate_ok(cfg: &BlockTimingCfg, ctx: SendCtx) {
    from_cfg(cfg).note_gate_ok(ctx);
}

pub(crate) fn note_seal(cfg: &BlockTimingCfg, ctx: SealCtx) {
    from_cfg(cfg).note_seal(ctx);
}

pub(crate) fn try_flush_once(cfg: &BlockTimingCfg) -> Result<(), String> {
    from_cfg(cfg).try_flush_once()
}

fn apply_op(rec: &mut PendRec, op: &DefOp) {
    match op.kind {
        OpKind::T0 => {
            if rec.t0_ms.is_none() {
                rec.t0_ms = op.t_ms;
            }
            if rec.nom_ms.is_none() {
                rec.nom_ms = Some(op.nom_ms as f64);
            }
            if rec.grid_ms.is_none() {
                rec.grid_ms = Some(op.grid_ms as f64);
            }
        }
        OpKind::Send => {
            rec.prop_send_n = rec.prop_send_n.saturating_add(1);
            if rec.prop_send_ms.is_none() {
                rec.prop_send_ms = op.t_ms;
            }
        }
        OpKind::AttRx => {
            if rec.att_rx_ms.is_none() {
                rec.att_rx_ms = op.t_ms;
            }
            if rec.att_id.is_none() {
                rec.att_id = op.att_id.clone();
            }
        }
        OpKind::AttProc => {
            if rec.att_proc_start_ms.is_none() {
                rec.att_proc_start_ms = op.t_ms;
            }
            if rec.att_proc_ms.is_none() {
                rec.att_proc_ms = op.proc_ms;
            }
        }
        OpKind::AttWire => {
            if rec.att_wire_ms.is_none() {
                rec.att_wire_ms = op.t_ms;
            }
            if rec.att_id.is_none() {
                rec.att_id = op.att_id.clone();
            }
        }
        OpKind::AttOk => {
            if rec.prop_att_ms.is_none() {
                rec.prop_att_ms = op.t_ms;
            }
            if rec.att_id.is_none() {
                rec.att_id = op.att_id.clone();
            }
        }
        OpKind::GateOk => {
            if rec.gate_ok_ms.is_none() {
                rec.gate_ok_ms = op.t_ms;
            }
        }
        OpKind::Seal => {}
    }
}

fn mk_row(cfg: &BlockTimingCfg, rec: &PendRec, op: &DefOp) -> RowOut {
    let seal_ms = op.t_ms.unwrap_or(0.0);
    let t0 = rec.t0_ms.unwrap_or(seal_ms);
    let d_ms = mk_dms(rec, t0, seal_ms, op.wall_total_ms);
    let profile = op
        .profile_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| Value::Object(Map::new()));
    RowOut {
        schema_v: 1,
        cluster_id: cfg.cluster_id.clone(),
        height: op.h,
        round: op.r,
        sealed_h: op.h,
        proposer_instance_id: cfg.prop_id.clone(),
        attester_instance_id: rec.att_id.clone(),
        t0_ms: t0.round() as u64,
        d_ms,
        pending_ticks_at_seal: op.pending_ticks,
        gate_recheck_used: op.gate_recheck,
        autosnapshot_checkpoint: op.autosnap,
        suppress_strike: op.supp_strike,
        attest_timeout: op.attest_to,
        nominal_seal_ms: rec.nom_ms.unwrap_or(op.nom_ms as f64).round() as u64,
        grid_deadline_ms: rec.grid_ms.unwrap_or(op.grid_ms as f64).round() as u64,
        seal_slip_ms: seal_ms.round() as i64
            - rec.grid_ms.unwrap_or(op.grid_ms as f64).round() as i64,
        profile,
        pwmd_marker: cfg.pwmd_marker.clone(),
    }
}

fn key_of(h: u64, r: u32) -> String {
    format!("{h}:{r}")
}

fn mk_dms(rec: &PendRec, t0: f64, seal_ms: f64, wall_total_ms: f64) -> DMs {
    let dm = |v: Option<f64>| v.map(|x| if x >= t0 { (x - t0).round() as u64 } else { 0 });
    DMs {
        prop_round_open: Some(0),
        prop_first_wire_send: dm(rec.prop_send_ms),
        prop_wire_resend_total: rec.prop_send_n,
        att_rx_propose: dm(rec.att_rx_ms),
        att_proc_start: dm(rec.att_proc_start_ms),
        att_proc: rec.att_proc_ms.map(|v| v.max(0.0).round() as u64),
        att_wire_send: dm(rec.att_wire_ms),
        prop_rx_attest: dm(rec.prop_att_ms),
        prop_gate_ready: dm(rec.gate_ok_ms),
        prop_seal_commit: Some(if seal_ms >= t0 {
            (seal_ms - t0).round() as u64
        } else {
            0
        }),
        wall_total: Some(wall_total_ms),
    }
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(dir)
        .map_err(|e| format!("block_timing mkdir failed path={}: {e}", dir.display()))
}

fn read_pend(path: &Path) -> Result<BTreeMap<String, PendRec>, String> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = fs::read_to_string(path).map_err(|e| {
        format!(
            "block_timing pending read failed path={}: {e}",
            path.display()
        )
    })?;
    serde_json::from_str(&raw).map_err(|e| {
        format!(
            "block_timing pending parse failed path={}: {e}",
            path.display()
        )
    })
}

fn write_pend(path: &Path, map: &mut BTreeMap<String, PendRec>) -> Result<(), String> {
    trim_pending_map_tail(map, PEND_MAX_RECORDS);
    let raw = serde_json::to_string_pretty(map)
        .map_err(|e| format!("block_timing pending encode failed: {e}"))?;
    fs::write(path, raw).map_err(|e| {
        format!(
            "block_timing pending write failed path={}: {e}",
            path.display()
        )
    })
}

fn trim_pending_map_tail(map: &mut BTreeMap<String, PendRec>, max_records: usize) {
    if map.len() <= max_records {
        return;
    }
    let mut ordered: Vec<(u64, u32, String)> = map
        .keys()
        .map(|k| {
            let (h, r) = parse_pending_key(k);
            (h, r, k.clone())
        })
        .collect();
    ordered.sort_unstable_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let drop_n = ordered.len().saturating_sub(max_records);
    for (_, _, key) in ordered.into_iter().take(drop_n) {
        map.remove(&key);
    }
}

fn parse_pending_key(key: &str) -> (u64, u32) {
    let mut parts = key.split(':');
    let h = parts
        .next()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let r = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    (h, r)
}

fn append_jsonl(cfg: &BlockTimingCfg, row: &RowOut) -> Result<(), String> {
    let mut out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.path)
        .map_err(|e| {
            format!(
                "block_timing jsonl open failed path={}: {e}",
                cfg.path.display()
            )
        })?;
    let mut line =
        serde_json::to_string(row).map_err(|e| format!("block_timing row encode failed: {e}"))?;
    line.push('\n');
    out.write_all(line.as_bytes()).map_err(|e| {
        format!(
            "block_timing jsonl write failed path={}: {e}",
            cfg.path.display()
        )
    })?;
    out.sync_all().map_err(|e| {
        format!(
            "block_timing jsonl sync failed path={}: {e}",
            cfg.path.display()
        )
    })?;
    trim_jsonl_tail(&cfg.path, 1500)
}

fn trim_jsonl_tail(path: &Path, max_rows: usize) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| {
        format!(
            "block_timing jsonl trim read failed path={}: {e}",
            path.display()
        )
    })?;
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() <= max_rows {
        return Ok(());
    }
    let start = lines.len().saturating_sub(max_rows);
    let mut trimmed = lines[start..].join("\n");
    trimmed.push('\n');
    fs::write(path, trimmed).map_err(|e| {
        format!(
            "block_timing jsonl trim write failed path={}: {e}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(1);

    fn mk_tmp_bt() -> BlockTiming {
        let mut p = std::env::temp_dir();
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let n = format!(
            "pwm-bt-nb-{}-{}-{}.jsonl",
            std::process::id(),
            crate::current_time_ms().unwrap_or(0),
            seq,
        );
        p.push(n);
        BlockTiming::mk_new(BlockTimingCfg::mk_new(
            true,
            p,
            "cy".to_string(),
            "cy-proposer".to_string(),
            "pwmd/test".to_string(),
        ))
    }

    #[test]
    fn dms_from_abs_ok() {
        let rec = PendRec {
            t0_ms: Some(1_000.0),
            prop_send_ms: Some(1_010.0),
            prop_send_n: 3,
            att_rx_ms: Some(1_050.0),
            att_proc_start_ms: Some(1_051.0),
            att_proc_ms: Some(4.0),
            att_wire_ms: Some(1_060.0),
            prop_att_ms: Some(1_080.0),
            gate_ok_ms: Some(1_090.0),
            ..PendRec::default()
        };
        let d = mk_dms(&rec, 1_000.0, 1_120.0, 120.0);
        assert_eq!(d.prop_first_wire_send, Some(10));
        assert_eq!(d.prop_wire_resend_total, 3);
        assert_eq!(d.att_proc, Some(4));
        assert_eq!(d.prop_gate_ready, Some(90));
        assert_eq!(d.wall_total, Some(120.0));
    }

    #[test]
    fn note_no_block_lock_busy() {
        let bt = mk_tmp_bt();
        ensure_parent(&bt.cfg.lock_path).expect("mkdir");
        let hold = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&bt.cfg.lock_path)
            .expect("open lock");
        hold.lock_exclusive().expect("hold lock");

        let t0 = std::time::Instant::now();
        bt.note_send(SendCtx {
            h: 10,
            r: 0,
            t_ms: 1.0,
        });
        let dt = t0.elapsed().as_millis();
        assert!(dt < 5, "note_send blocked for {dt}ms");
        assert_eq!(bt.queue_depth(), 1);

        let _ = fs2::FileExt::unlock(&hold);
        bt.try_flush_once().expect("flush");
        assert_eq!(bt.queue_depth(), 0);
    }

    #[test]
    fn seal_flush_row_ok() {
        let bt = mk_tmp_bt();
        bt.note_t0(T0Ctx {
            h: 42,
            r: 0,
            t_ms: 1_000.0,
            grid_ms: 1_000,
            nom_ms: 1_000,
        });
        bt.note_send(SendCtx {
            h: 42,
            r: 0,
            t_ms: 1_010.0,
        });
        bt.note_seal(SealCtx {
            h: 42,
            r: 0,
            seal_ms: 1_120.0,
            wall_total_ms: 120.0,
            pending_ticks: 7,
            gate_recheck: true,
            autosnap: false,
            supp_strike: false,
            attest_to: false,
            nom_ms: 1_000,
            grid_ms: 1_000,
            profile_json: ProfileTime::default().json_stats(""),
        });
        let raw = fs::read_to_string(&bt.cfg.path).expect("jsonl");
        assert!(raw.contains("\"schema_v\":1"));
        assert!(raw.contains("\"height\":42"));
    }

    #[test]
    fn json_stats_merge_schema() {
        let mut pt = ProfileTime::default();
        pt.start(Some(1_000));
        pt.checkpoint_at("gate_ok", 1_025);
        pt.checkpoint_at("seal_done", 1_040);
        let out = pt.json_stats("{\"scope\":\"seal\"}");
        let v: Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["scope"], "seal");
        assert_eq!(v["start_ms"], 1_000);
        assert_eq!(v["checkpoints_rel_ms"]["gate_ok"], 25.0);
        assert_eq!(v["checkpoints_rel_ms"]["seal_done"], 40.0);
    }

    #[test]
    fn round_ms_precision_applies_digits() {
        assert_eq!(round_ms(9.0, 2), 9.0);
        assert_eq!(round_ms(1.234, 2), 1.23);
        assert_eq!(round_ms(1.235, 2), 1.24);
        assert_eq!(round_ms(12.3456, 3), 12.346);
    }

    #[test]
    fn jsonl_tail_keeps_latest() {
        let mut p = std::env::temp_dir();
        let seq = TEST_SEQ.fetch_add(1, Ordering::Relaxed);
        p.push(format!(
            "pwm-bt-trim-{}-{}-{}.jsonl",
            std::process::id(),
            crate::current_time_ms().unwrap_or(0),
            seq
        ));

        let mut content = String::new();
        for i in 0..1505usize {
            content.push_str(&format!("{{\"n\":{i}}}\n"));
        }
        fs::write(&p, content).expect("seed jsonl");

        trim_jsonl_tail(&p, 1500).expect("trim");
        let raw = fs::read_to_string(&p).expect("trimmed jsonl");
        let lines: Vec<&str> = raw.lines().collect();
        assert_eq!(lines.len(), 1500);
        assert!(lines.first().is_some_and(|line| line.contains("\"n\":5")));
        assert!(lines.last().is_some_and(|line| line.contains("\"n\":1504")));
    }

    #[test]
    fn pendrec_serialize_2dp() {
        let rec = PendRec {
            t0_ms: Some(1000.0),
            prop_send_ms: Some(1010.0),
            prop_send_n: 2,
            att_rx_ms: Some(1050.0),
            att_proc_start_ms: Some(1051.0),
            att_proc_ms: Some(4.0),
            att_wire_ms: Some(1060.0),
            prop_att_ms: Some(1080.0),
            gate_ok_ms: Some(1090.0),
            att_id: Some("att-1".to_string()),
            nom_ms: Some(1000.0),
            grid_ms: Some(1000.0),
        };
        let raw = serde_json::to_string_pretty(&rec).expect("serialize pendrec");
        assert!(raw.contains("\"t0_ms\": \"1000.00\""));
        assert!(raw.contains("\"att_proc_ms\": \"4.00\""));
    }

    #[test]
    fn pendrec_parse_mixed_ms() {
        let raw = r#"
        {
            "t0_ms": "1000.00",
            "prop_send_ms": 1010.5,
            "prop_send_n": 2,
            "att_rx_ms": 1050,
            "att_proc_start_ms": "1051",
            "att_proc_ms": "4.00",
            "att_wire_ms": 1060,
            "prop_att_ms": "1080.00",
            "gate_ok_ms": 1090,
            "att_id": "att-1",
            "nom_ms": "1000.00",
            "grid_ms": 1000
        }
        "#;
        let rec: PendRec = serde_json::from_str(raw).expect("parse pendrec");
        assert_eq!(rec.t0_ms, Some(1000.0));
        assert_eq!(rec.prop_send_ms, Some(1010.5));
        assert_eq!(rec.att_proc_ms, Some(4.0));
    }

    #[test]
    fn pending_tail_keeps_high() {
        let mut map = BTreeMap::new();
        for h in 1u64..=10 {
            map.insert(format!("{h}:0"), PendRec::default());
        }
        trim_pending_map_tail(&mut map, 4);
        assert_eq!(map.len(), 4);
        assert!(map.contains_key("7:0"));
        assert!(map.contains_key("8:0"));
        assert!(map.contains_key("9:0"));
        assert!(map.contains_key("10:0"));
    }
}
