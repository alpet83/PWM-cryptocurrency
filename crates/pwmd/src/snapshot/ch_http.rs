//! Row keys, HTTP URL normalization, and optional ClickHouse snapshot I/O (`clickhouse-snapshot`).
//! `ch_load` records HTTP fetch vs local parse (blocks_table vs legacy_snapshot branch).

#![cfg_attr(not(feature = "clickhouse-snapshot"), allow(dead_code))]

use crate::identity::RuntimeIdentity;

#[cfg(feature = "clickhouse-snapshot")]
use super::telemetry::{ChSnapTiming, SNAP_STARTUP_TARGET};
#[cfg(feature = "clickhouse-snapshot")]
use crate::snapshot::epoch::SNAP_CHK_BLK_IV;
#[cfg(feature = "clickhouse-snapshot")]
use crate::snapshot::io::{decode_snapshot_txt, encode_snap_data_txt, load_snapshot};
#[cfg(feature = "clickhouse-snapshot")]
use crate::snapshot::snapshot_genesis_accounts;
use crate::snapshot::types::roaming_to_wire;
#[cfg(feature = "clickhouse-snapshot")]
use crate::snapshot::types::{BlocksStored, SnapshotData};
#[cfg(feature = "clickhouse-snapshot")]
use crate::snapshot::SNAPSHOT_VERSION;
#[cfg(feature = "clickhouse-snapshot")]
use crate::state::Inner;
#[cfg(feature = "clickhouse-snapshot")]
use pwm_core::block::{hdr_hash, Block};
#[cfg(feature = "clickhouse-snapshot")]
use pwm_core::digest;
#[cfg(feature = "clickhouse-snapshot")]
use pwm_core::genesis::GenCfg;
#[cfg(feature = "clickhouse-snapshot")]
use pwm_core::hd::domain_of_account_id;
#[cfg(feature = "clickhouse-snapshot")]
use pwm_core::State;
#[cfg(feature = "clickhouse-snapshot")]
use reqwest::blocking::Client;
#[cfg(feature = "clickhouse-snapshot")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "clickhouse-snapshot")]
use std::collections::BTreeMap;
#[cfg(feature = "clickhouse-snapshot")]
use std::path::Path;
#[cfg(feature = "clickhouse-snapshot")]
use std::sync::Once;
#[cfg(feature = "clickhouse-snapshot")]
use std::time::Instant;
#[cfg(feature = "clickhouse-snapshot")]
use tracing::warn;
use url::Url;

/// Wire HTTP params + row key for autosnapshot prototypes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapChCfg {
    pub http_base: String,
    pub database: String,
    pub table_blocks: String,
    pub table_checkpoints: String,
    /// Append-only validator acceptance (`validators_accept__*` DDL).
    pub table_validators_accept: String,
    /// Legacy monolithic `snapshot_json` row (`import_snapshot_file`, migration reads).
    pub legacy_snapshot_table: String,
    pub row_key: String,
    /// When set, failed ClickHouse writes fall back to JsonFile at this path (`--data-file`).
    pub json_fallback: Option<std::path::PathBuf>,
}

fn is_safe_snap_seg(s: &str) -> bool {
    let max = 512usize;
    if s.is_empty() || s.len() > max {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '|' | '+' | ':'))
}

/// Stable row identity: override or `{network_id}|0x{domain_hi}|{genesis_digest}` (no node/cluster in default).
pub fn pwmd_snap_row_key(
    override_: Option<&str>,
    genesis_st0_digest_hex: &str,
    id: &RuntimeIdentity,
) -> Result<String, String> {
    if let Some(raw) = override_ {
        let t = raw.trim();
        if !t.is_empty() {
            return if is_safe_snap_seg(t) {
                Ok(t.to_string())
            } else {
                Err(
                    "snapshot-store-key contains disallowed chars (use ascii alnum plus ._-|+:)"
                        .into(),
                )
            };
        }
    }
    let d = genesis_st0_digest_hex.trim();
    if d.is_empty() || !is_safe_snap_seg(d) {
        return Err("genesis digest hex for snapshot row key is empty or invalid".into());
    }
    let nid = id.network_id.as_str();
    if !is_safe_snap_seg(nid) {
        return Err("identity fields contain disallowed chars for snapshot row key".into());
    }
    Ok(format!("{}|0x{:02x}|{}", nid, id.cluster_domain_hi, d))
}

/// Database identifier derived from `network_id` (ascii alnum + underscore).
pub fn snap_ch_db_net(network_id: &str) -> String {
    let t: String = network_id
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if t.is_empty() {
        "pwm_net".to_string()
    } else {
        t
    }
}

/// `clickhouse_database == ""` or default `pwm_snapshots` resolves to [`snap_ch_db_net`].
pub fn resolve_ch_database(clickhouse_database: &str, network_id: &str) -> Result<String, String> {
    let t = clickhouse_database.trim();
    if t.is_empty() || t == "pwm_snapshots" {
        let d = snap_ch_db_net(network_id);
        snap_ch_sql_id(&d)?;
        return Ok(d);
    }
    snap_ch_sql_id(t)?;
    Ok(t.to_string())
}

/// Table names `blocks__0xHH` / `checkpoints__0xHH` from domain high byte, unless `clickhouse_table` overrides stem.
pub fn snap_ch_tbl_pair(domain_hi: u8, clickhouse_table: &str) -> (String, String) {
    let stem = snap_ch_tbl_stem(domain_hi, clickhouse_table);
    (
        format!("blocks__{}", stem),
        format!("checkpoints__{}", stem),
    )
}

fn snap_ch_tbl_stem(domain_hi: u8, clickhouse_table: &str) -> String {
    let stem = clickhouse_table.trim();
    if stem.is_empty() || stem == "node_snapshot" {
        format!("0x{:02x}", domain_hi)
    } else {
        stem.to_string()
    }
}

/// `validators_accept__0xHH` stem aligned with [`snap_ch_tbl_pair`] / [`snap_ch_tbl_stem`].
pub fn snap_ch_tbl_validators(domain_hi: u8, clickhouse_table: &str) -> String {
    format!(
        "validators_accept__{}",
        snap_ch_tbl_stem(domain_hi, clickhouse_table)
    )
}

/// Trims slashes and ensures URL has an HTTP scheme with a host.
pub fn norm_ch_http_base(raw: &str) -> Result<String, String> {
    let t = raw.trim().trim_end_matches('/');
    if t.is_empty() {
        return Err("clickhouse-url is empty".into());
    }
    let parsed = Url::parse(t).map_err(|e| format!("invalid clickhouse-url: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("clickhouse-url must use http or https scheme".into());
    }
    if parsed.host_str().is_none() {
        return Err("clickhouse-url must include a host".into());
    }
    Ok(t.to_string())
}

/// Validates database/table fragments embedded into ClickHouse queries.
pub fn snap_ch_sql_id(s: &str) -> Result<(), String> {
    let t = s.trim();
    if t.is_empty() || t.len() > 128 {
        return Err("clickhouse database/table identifier empty or too long".into());
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_'))
    {
        return Err("clickhouse database/table must be ascii alphanumeric or underscore".into());
    }
    Ok(())
}

#[cfg(feature = "clickhouse-snapshot")]
static VALIDATORS_ACCEPT_DEFERRED: Once = Once::new();

/// Warn once: `validators_accept` rows are not written until consensus signing is wired (Wave 4 stub).
#[cfg(feature = "clickhouse-snapshot")]
fn warn_validators_accept_deferred() {
    VALIDATORS_ACCEPT_DEFERRED.call_once(|| {
        warn!(
            target: "pwmd::snapshot",
            "validators_accept INSERT deferred (validator id + signature path); checkpoint_digest for signing will use hex(pwm_core::digest(state)) — docs/reviews/sprint-15-slice-7-plan.md §6.1"
        );
    });
}

/// JSON map `"0xHH"` → decimal string of Σ(balance_pwm + staked) per account domain_hi (checkpoint seal only).
#[cfg(feature = "clickhouse-snapshot")]
fn encode_shard_balance_json(st: &State) -> Result<String, String> {
    let mut sums: BTreeMap<String, u128> = BTreeMap::new();
    for (id, acc) in &st.accounts {
        let hi = domain_of_account_id(id).to_be_bytes()[0];
        let k = format!("0x{:02x}", hi);
        let row = sums.entry(k).or_insert(0);
        *row = row.saturating_add(acc.balance_pwm.saturating_add(acc.staked_pwm_raw));
    }
    let obj: serde_json::Map<String, serde_json::Value> = sums
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v.to_string())))
        .collect();
    serde_json::to_string(&serde_json::Value::Object(obj))
        .map_err(|e| format!("shard_balance json: {e}"))
}

#[cfg(feature = "clickhouse-snapshot")]
fn truncate_body(b: &str) -> String {
    const CAP: usize = 512;
    if b.len() <= CAP {
        b.to_string()
    } else {
        format!("{}…", &b[..CAP])
    }
}

#[cfg(feature = "clickhouse-snapshot")]
fn mk_client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("snap ch blocking client builder")
}

#[cfg(feature = "clickhouse-snapshot")]
impl SnapChCfg {
    /// HTTP `INSERT` of one JSONEachRow line — legacy monolithic snapshot row.
    pub fn ch_insert_snapshot_json(&self, snapshot_json: &str) -> Result<(), String> {
        #[derive(Serialize)]
        struct InsRow<'a> {
            row_key: &'a str,
            snapshot_json: &'a str,
        }
        let row = InsRow {
            row_key: self.row_key.as_str(),
            snapshot_json,
        };
        let line = serde_json::to_string(&row).map_err(|e| format!("snap ch insert row: {e}"))?;
        let q = format!(
            "INSERT INTO `{}`.`{}` (row_key, snapshot_json) FORMAT JSONEachRow",
            self.database.replace('`', ""),
            self.legacy_snapshot_table.replace('`', "")
        );
        self.ch_post_json_row(&q, &(line + "\n"))
    }

    fn ch_post_json_row(&self, insert_query: &str, body: &str) -> Result<(), String> {
        let mut api = Url::parse(&format!("{}/", self.http_base.trim_end_matches('/')))
            .map_err(|e| format!("snap ch url parse: {e}"))?;
        api.query_pairs_mut()
            .append_pair("database", &self.database)
            .append_pair("query", insert_query);
        let cli = mk_client();
        let resp = cli
            .post(api)
            .body(body.to_string())
            .send()
            .map_err(|e| format!("clickhouse snapshot INSERT: transport {e}"))?;
        let status = resp.status();
        let tb = resp.text().unwrap_or_else(|_| "<no body>".into());
        if !status.is_success() {
            return Err(format!(
                "clickhouse snapshot INSERT http {} body={}",
                status.as_u16(),
                truncate_body(&tb)
            ));
        }
        Ok(())
    }

    fn ch_get_body(&self, select_query: &str, param_rk: &str) -> Result<String, String> {
        let mut api = Url::parse(&format!("{}/", self.http_base.trim_end_matches('/')))
            .map_err(|e| format!("snap ch url parse: {e}"))?;
        api.set_query(Some(""));
        api.query_pairs_mut()
            .append_pair("database", &self.database)
            .append_pair("query", select_query)
            .append_pair("param_rk", param_rk);
        let cli = mk_client();
        let resp = cli
            .get(api)
            .send()
            .map_err(|e| format!("clickhouse snapshot SELECT: transport {e}"))?;
        let status = resp.status();
        let body = resp
            .text()
            .map_err(|e| format!("clickhouse snapshot SELECT body: {e}"))?;
        if !status.is_success() {
            return Err(format!(
                "clickhouse snapshot SELECT http {} body={}",
                status.as_u16(),
                truncate_body(&body)
            ));
        }
        Ok(body)
    }

    /// Migration bridge: INSERT per block + tip checkpoint + legacy monolithic row for old readers.
    pub fn import_snapshot_file(&self, path: &Path, cfg: &GenCfg) -> Result<(), String> {
        let Some(snap) = load_snapshot(path, cfg)? else {
            return Err(format!(
                "snapshot file missing or empty: {}",
                path.display()
            ));
        };
        for blk in &snap.blocks {
            self.ch_insert_block_row(cfg, blk)?;
        }
        let gdh = hex::encode(digest(&cfg.state0()));
        let tip = snap.blocks.last().map(|b| b.hdr.height).unwrap_or(0);
        if tip > 0 {
            let tip_sr = snap
                .blocks
                .last()
                .map(|b| hex::encode(b.hdr.state_root))
                .unwrap_or_default();
            self.ch_insert_checkpoint_row(
                &gdh,
                tip,
                &tip_sr,
                &snap.state,
                &snap.roaming,
                &snap.cross_shard,
            )?;
            let mut h = SNAP_CHK_BLK_IV;
            while h < tip {
                let st = replay_state_at(cfg, &snap.blocks, h)?;
                let sr = snap
                    .blocks
                    .iter()
                    .find(|b| b.hdr.height == h)
                    .map(|b| hex::encode(b.hdr.state_root))
                    .unwrap_or_default();
                self.ch_insert_checkpoint_row(&gdh, h, &sr, &st, &snap.roaming, &snap.cross_shard)?;
                h += SNAP_CHK_BLK_IV;
            }
        }
        let txt = encode_snap_data_txt(&snap)?;
        self.ch_insert_snapshot_json(&txt)?;
        Ok(())
    }

    fn ch_insert_block_row(&self, gcfg: &GenCfg, blk: &Block) -> Result<(), String> {
        #[derive(Serialize)]
        struct BlockIns<'a> {
            row_key: &'a str,
            height: u64,
            block_hash: String,
            prev_hash: String,
            ts: u64,
            prod_idx: u32,
            tx_count: u64,
            state_root: String,
            payload_json: String,
        }
        let prod = gcfg
            .vals
            .set
            .get(blk.hdr.prod_idx as usize)
            .ok_or_else(|| format!("block height {} bad prod_idx", blk.hdr.height))?;
        if !blk.hdr.verify_sig(&prod.pubkey) {
            return Err(format!(
                "clickhouse block import: invalid sig at height {}",
                blk.hdr.height
            ));
        }
        let payload_json =
            serde_json::to_string(blk).map_err(|e| format!("block payload json: {e}"))?;
        let row = BlockIns {
            row_key: self.row_key.as_str(),
            height: blk.hdr.height,
            block_hash: hex::encode(hdr_hash(&blk.hdr)),
            prev_hash: hex::encode(blk.hdr.prev_hash),
            ts: blk.hdr.ts,
            prod_idx: blk.hdr.prod_idx,
            tx_count: blk.txs.len() as u64,
            state_root: hex::encode(blk.hdr.state_root),
            payload_json,
        };
        let line = serde_json::to_string(&row).map_err(|e| format!("block insert row: {e}"))?;
        let q = format!(
            "INSERT INTO `{}`.`{}` (row_key, height, block_hash, prev_hash, ts, prod_idx, tx_count, state_root, payload_json) FORMAT JSONEachRow",
            self.database.replace('`', ""),
            self.table_blocks.replace('`', "")
        );
        self.ch_post_json_row(&q, &(line + "\n"))
    }

    fn ch_insert_checkpoint_row(
        &self,
        genesis_digest: &str,
        checkpoint_height: u64,
        state_root_hex: &str,
        st: &State,
        roaming: &crate::snapshot::types::SnapshotRoamingWire,
        cross: &crate::ledger::CrossShardLedger,
    ) -> Result<(), String> {
        #[derive(Serialize)]
        struct Row<'a> {
            row_key: &'a str,
            genesis_digest: &'a str,
            checkpoint_height: u64,
            state_root: &'a str,
            state_json: String,
            roaming_json: String,
            cross_shard_json: String,
            /// Checkpoint-level aggregate for explorer; formula in `docs/reviews/sprint-15-slice-7-plan.md` §6.1.
            shard_balance: String,
        }
        let state_json =
            serde_json::to_string(st).map_err(|e| format!("checkpoint state_json: {e}"))?;
        let roaming_json =
            serde_json::to_string(roaming).map_err(|e| format!("checkpoint roaming: {e}"))?;
        let cross_shard_json =
            serde_json::to_string(cross).map_err(|e| format!("checkpoint cross_shard: {e}"))?;
        let shard_balance = encode_shard_balance_json(st)?;
        let row = Row {
            row_key: self.row_key.as_str(),
            genesis_digest,
            checkpoint_height,
            state_root: state_root_hex,
            state_json,
            roaming_json,
            cross_shard_json,
            shard_balance,
        };
        let line = serde_json::to_string(&row).map_err(|e| format!("chk insert row: {e}"))?;
        let q = format!(
            "INSERT INTO `{}`.`{}` (row_key, genesis_digest, checkpoint_height, state_root, state_json, roaming_json, cross_shard_json, shard_balance) FORMAT JSONEachRow",
            self.database.replace('`', ""),
            self.table_checkpoints.replace('`', "")
        );
        self.ch_post_json_row(&q, &(line + "\n"))?;
        warn_validators_accept_deferred();
        Ok(())
    }

    /// Seal path: append one block row; checkpoint every [`SNAP_CHK_BLK_IV`].
    pub(crate) fn ch_save_seal(&self, inner: &Inner) -> Result<(), String> {
        let blk = inner
            .chain
            .blocks
            .back()
            .ok_or_else(|| "clickhouse seal: no tip block".to_string())?;
        self.ch_insert_block_row(&inner.chain.cfg, blk)?;
        let h = inner.chain.tip_h();
        if h > 0 && h % SNAP_CHK_BLK_IV == 0 {
            let gdh = hex::encode(digest(&inner.chain.cfg.state0()));
            let sr = hex::encode(blk.hdr.state_root);
            self.ch_insert_checkpoint_row(
                &gdh,
                h,
                &sr,
                &inner.chain.st,
                &roaming_to_wire(&inner.roaming_pool),
                &inner.cross_shard,
            )?;
        }
        Ok(())
    }

    /// Tip summary without a new block (relay): checkpoint-style row at current height.
    pub(crate) fn ch_save_tip_summary(&self, inner: &Inner) -> Result<(), String> {
        let h = inner.chain.tip_h();
        if h == 0 {
            return Ok(());
        }
        let gdh = hex::encode(digest(&inner.chain.cfg.state0()));
        let sr = inner
            .chain
            .blocks
            .back()
            .map(|b| hex::encode(b.hdr.state_root))
            .unwrap_or_default();
        self.ch_insert_checkpoint_row(
            &gdh,
            h,
            &sr,
            &inner.chain.st,
            &roaming_to_wire(&inner.roaming_pool),
            &inner.cross_shard,
        )
    }

    pub(crate) fn ch_load(
        &self,
        gcfg: &GenCfg,
    ) -> Result<(Option<SnapshotData>, ChSnapTiming), String> {
        let mut t = ChSnapTiming::default();
        let q = format!(
            "SELECT payload_json FROM `{}`.`{}` WHERE row_key = {{rk:String}} ORDER BY height ASC FORMAT JSONEachRow",
            self.database.replace('`', ""),
            self.table_blocks.replace('`', "")
        );
        let t_http = Instant::now();
        let body = self.ch_get_body(&q, &self.row_key).map_err(|e| {
            warn!(
                target: SNAP_STARTUP_TARGET,
                stage = "ch_http",
                branch = "blocks_table",
                err = %e,
                "clickhouse snapshot load failed"
            );
            e
        })?;
        t.http_ms = t_http.elapsed().as_millis() as u64;
        if !body.trim().is_empty() {
            t.branch = "blocks_table";
            let t_parse = Instant::now();
            let out = self.ch_load_blk_lines(gcfg, &body).map_err(|e| {
                warn!(
                    target: SNAP_STARTUP_TARGET,
                    stage = "ch_parse",
                    branch = "blocks_table",
                    err = %e,
                    "clickhouse snapshot load failed"
                );
                e
            })?;
            t.parse_ms = t_parse.elapsed().as_millis() as u64;
            return Ok((out, t));
        }
        let q2 = format!(
            "SELECT snapshot_json FROM `{}`.`{}` WHERE row_key = {{rk:String}} ORDER BY inserted_at DESC LIMIT 1 FORMAT JSONEachRow",
            self.database.replace('`', ""),
            self.legacy_snapshot_table.replace('`', "")
        );
        let t_http2 = Instant::now();
        let body2 = self.ch_get_body(&q2, &self.row_key).map_err(|e| {
            warn!(
                target: SNAP_STARTUP_TARGET,
                stage = "ch_http",
                branch = "legacy_snapshot",
                err = %e,
                "clickhouse snapshot load failed"
            );
            e
        })?;
        t.http_ms += t_http2.elapsed().as_millis() as u64;
        t.branch = "legacy_snapshot";
        let t_parse = Instant::now();
        let out = ch_parse_legacy_body(gcfg, &body2).map_err(|e| {
            warn!(
                target: SNAP_STARTUP_TARGET,
                stage = "ch_parse",
                branch = "legacy_snapshot",
                err = %e,
                "clickhouse snapshot load failed"
            );
            e
        })?;
        t.parse_ms = t_parse.elapsed().as_millis() as u64;
        Ok((out, t))
    }

    fn ch_load_blk_lines(&self, gcfg: &GenCfg, body: &str) -> Result<Option<SnapshotData>, String> {
        let mut blocks: Vec<Block> = Vec::new();
        for line in body.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            #[derive(Deserialize)]
            struct PayloadRow {
                payload_json: String,
            }
            let pr: PayloadRow =
                serde_json::from_str(t).map_err(|e| format!("ch blocks row: {e}"))?;
            let blk: Block = serde_json::from_str(&pr.payload_json)
                .map_err(|e| format!("ch block json: {e}"))?;
            blocks.push(blk);
        }
        let tip = blocks.last().map(|b| b.hdr.height).unwrap_or(0);
        let snap = SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(gcfg),
            genesis_anchor: None,
            blocks,
            state: gcfg.state0(),
            roaming: Default::default(),
            cross_shard: Default::default(),
            blocks_stored: BlocksStored::Epochs,
            checkpoint_height: tip,
        };
        let txt = encode_snap_data_txt(&snap)?;
        let mut out = decode_snapshot_txt(&txt, gcfg)?;
        if let Some(ref mut s) = out {
            s.blocks_stored = BlocksStored::Inline;
            s.checkpoint_height = 0;
        }
        Ok(out)
    }
}

#[cfg(feature = "clickhouse-snapshot")]
fn ch_parse_legacy_body(gcfg: &GenCfg, body: &str) -> Result<Option<SnapshotData>, String> {
    let row = body.trim();
    if row.is_empty() {
        return Ok(None);
    }
    #[derive(Deserialize)]
    struct Jrow {
        snapshot_json: String,
    }
    let jr: Jrow = serde_json::from_str(row).map_err(|e| {
        format!(
            "clickhouse legacy json row decode: {e} row={}",
            truncate_body(row)
        )
    })?;
    decode_snapshot_txt(&jr.snapshot_json, gcfg)
}

#[cfg(feature = "clickhouse-snapshot")]
fn replay_state_at(gcfg: &GenCfg, blocks: &[Block], up_to_h: u64) -> Result<State, String> {
    let mut st = gcfg.state0();
    for blk in blocks {
        if blk.hdr.height > up_to_h {
            break;
        }
        st.refund_exp_locks(blk.hdr.height);
        for tx in &blk.txs {
            st.apply_tx_with_ctx(tx, blk.hdr.height, blk.hdr.ts, gcfg)
                .map_err(|e| format!("replay checkpoint state at {up_to_h}: {e}"))?;
        }
        st.refund_exp_locks(blk.hdr.height);
        st.drain_conservation_at_height(blk.hdr.height, gcfg);
        let prod_acct = gcfg.prod_acct(blk.hdr.prod_idx);
        if gcfg.is_legacy_policy() {
            st.reward_producer(&prod_acct, gcfg.block_reward);
        } else {
            let season_ppm = gcfg.season_ppm(blk.hdr.ts);
            st.reward_producer_v2(
                &prod_acct,
                gcfg.block_reward,
                gcfg.pwm_stake_min,
                season_ppm,
            );
        }
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::{norm_ch_http_base, pwmd_snap_row_key};
    use crate::identity::{DevLane, RuntimeIdentity, RuntimeIdentityMode};

    #[cfg(all(test, feature = "clickhouse-snapshot"))]
    use pwm_core::hd::domain_of_account_id;
    #[cfg(all(test, feature = "clickhouse-snapshot"))]
    use pwm_core::tx::{SignedTx, TxBody};
    #[cfg(all(test, feature = "clickhouse-snapshot"))]
    use url::Url;

    fn dev_id() -> RuntimeIdentity {
        RuntimeIdentity {
            network_id: "devnet".into(),
            cluster_domain_hi: 0x11,
            cluster_id: "c1".into(),
            node_id: "n1".into(),
            mode: RuntimeIdentityMode::Explicit,
        }
    }

    #[test]
    fn ch_base_trims_ok() {
        assert_eq!(
            norm_ch_http_base(" http://127.0.0.1:8123/ ").unwrap(),
            "http://127.0.0.1:8123"
        );
    }

    #[test]
    fn ch_base_https_ok() {
        assert_eq!(
            norm_ch_http_base("https://ch.example:8443").unwrap(),
            "https://ch.example:8443"
        );
    }

    #[test]
    fn ch_base_rejects_scheme() {
        assert!(norm_ch_http_base("ftp://127.0.0.1:8123").is_err());
    }

    #[test]
    fn row_key_derived_fmt() {
        let k = pwmd_snap_row_key(None, "abc123", &dev_id()).unwrap();
        assert_eq!(k, "devnet|0x11|abc123");
    }

    #[test]
    fn row_key_override_wins() {
        let k = pwmd_snap_row_key(Some("  my_key_1  "), "ignored", &dev_id()).unwrap();
        assert_eq!(k, "my_key_1");
    }

    #[test]
    fn row_key_rejects_bad_override() {
        assert!(pwmd_snap_row_key(Some("bad value"), "x", &dev_id()).is_err());
    }

    #[test]
    fn row_key_explicit_node_ok() {
        let id = RuntimeIdentity {
            network_id: "devnet".into(),
            cluster_domain_hi: 0x10,
            cluster_id: "dev-cluster-0x10".into(),
            node_id: "dev-node-0x10".into(),
            mode: RuntimeIdentityMode::Explicit,
        };
        let k = pwmd_snap_row_key(None, "deadbeef", &id).unwrap();
        assert!(k.ends_with("deadbeef"));
    }

    #[test]
    fn ch_sql_id_ok() {
        super::snap_ch_sql_id("pwm_snapshots").unwrap();
        assert!(super::snap_ch_sql_id("bad-name!").is_err());
    }

    #[test]
    fn tbl_validators_suffix_matches_blocks() {
        let hi = 0x2cu8;
        let (blocks, _) = super::snap_ch_tbl_pair(hi, "");
        let stem = blocks.strip_prefix("blocks__").expect("stem");
        assert_eq!(
            super::snap_ch_tbl_validators(hi, ""),
            format!("validators_accept__{}", stem)
        );
    }

    /// Empty accounts ⇒ `{}` shard_balance JSON (Wave 4 §6.1).
    #[cfg(feature = "clickhouse-snapshot")]
    #[test]
    fn shard_bal_json_empty() {
        let st = pwm_core::State::default();
        assert_eq!(super::encode_shard_balance_json(&st).unwrap(), "{}");
    }

    #[cfg(feature = "clickhouse-snapshot")]
    #[test]
    fn replay_state_uses_blk_ctx() {
        let (cfg, sks) = pwm_core::dev_net();
        let mut chain = pwm_core::Chain::boot(cfg.clone(), sks.clone());
        let signer = cfg.accounts[0].acct;
        let tx = SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&signer),
            cfg.accounts[0].der_idx,
            0,
            TxBody::Stake { amount: 1 },
        );
        chain.seal(vec![tx]).expect("seal");
        let blocks = chain.blocks.iter().cloned().collect::<Vec<_>>();
        let st = super::replay_state_at(&cfg, &blocks, 1).expect("replay");
        let acc = st.get(&signer).expect("signer account");
        assert_eq!(acc.marks_last_block, 1);
    }

    /// No-op unless `PWM_CLICKHOUSE_TEST_URL` is set (`clickhouse-snapshot` only).
    #[cfg(all(test, feature = "clickhouse-snapshot"))]
    #[test]
    fn ch_ping_env() {
        let Ok(url) = std::env::var("PWM_CLICKHOUSE_TEST_URL") else {
            return;
        };
        let base = norm_ch_http_base(&url).expect("norm url");
        let u =
            Url::parse(&format!("{}/ping", base.trim_end_matches('/'))).expect("parse ping url");
        let cli = super::mk_client();
        let resp = cli.get(u).send().expect("ping send");
        assert!(resp.status().is_success(), "ch ping http {}", resp.status());
        let body = resp.text().expect("ping body");
        assert!(
            body.trim_start().starts_with("Ok."),
            "ch ping body: {body:?}"
        );
    }
}
