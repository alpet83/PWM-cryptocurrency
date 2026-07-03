//! Append-only wallet transaction journal files.

use crate::TX_HISTORY_DIR;
use pwm_core::{account_id_to_human, AccountId};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JournalEntry {
    pub v: u8,
    pub ts: u64,
    pub kind: String,
    pub to: String,
    pub amount_pwm: String,
    pub fee_pwm: String,
    pub nonce: u64,
    pub status: String,
    pub execute_at_height: Option<u64>,
}

impl JournalEntry {
    pub fn pending(
        kind: &str,
        to: String,
        amount_pwm: String,
        fee_pwm: String,
        nonce: u64,
    ) -> Self {
        Self {
            v: 1,
            ts: now_secs(),
            kind: kind.to_string(),
            to,
            amount_pwm,
            fee_pwm,
            nonce,
            status: "pending".to_string(),
            execute_at_height: None,
        }
    }

    pub fn status_update(nonce: u64) -> Self {
        Self {
            v: 1,
            ts: now_secs(),
            kind: "status_update".to_string(),
            to: String::new(),
            amount_pwm: String::new(),
            fee_pwm: String::new(),
            nonce,
            status: "ok".to_string(),
            execute_at_height: None,
        }
    }
}

pub fn append_tx(wallet_dir: &Path, pretty_addr: &str, entry: &JournalEntry) -> io::Result<()> {
    let history_dir = wallet_dir.join(TX_HISTORY_DIR);
    fs::create_dir_all(&history_dir)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_dir.join(make_file_name(pretty_addr)))?;
    let line =
        serde_json::to_string(entry).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    writeln!(file, "{line}")
}

pub fn read_journal(wallet_dir: &Path, pretty_addr: &str) -> Vec<JournalEntry> {
    let path = wallet_dir
        .join(TX_HISTORY_DIR)
        .join(make_file_name(pretty_addr));
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            continue;
        };
        if let Ok(entry) = serde_json::from_str(&line) {
            entries.push(entry);
        }
    }
    entries
}

pub fn make_journal_filename(id_hex: &str) -> String {
    pretty_from_hex(id_hex)
        .map(|pretty| make_file_name(&pretty))
        .unwrap_or_else(|| make_file_name(&fallback_stem(id_hex)))
}

fn make_file_name(pretty_addr: &str) -> String {
    let stem = pretty_addr
        .trim()
        .trim_end_matches(".jsonl")
        .replace(':', "_")
        .replace(['\\', '/'], "-");
    if stem.is_empty() {
        "unknown.jsonl".to_string()
    } else {
        format!("{stem}.jsonl")
    }
}

fn pretty_from_hex(id_hex: &str) -> Option<String> {
    let raw = hex::decode(id_hex.trim()).ok()?;
    if raw.len() != 32 {
        return None;
    }
    let mut id: AccountId = [0u8; 32];
    id.copy_from_slice(&raw);
    Some(account_id_to_human(&id))
}

fn fallback_stem(id_hex: &str) -> String {
    let stem: String = id_hex
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(16)
        .collect();
    if stem.is_empty() {
        "unknown".to_string()
    } else {
        stem
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{append_tx, make_journal_filename, read_journal, JournalEntry};
    use std::fs;

    #[test]
    fn filename_from_hex() {
        let id_hex = "2c7e0000000000000000000000000000000000000000000000000000000000aa";

        assert_eq!(
            make_journal_filename(id_hex),
            "pwm1-CY-7E-f00000000-t00000000000000000000000000000000000000000000000000aa.jsonl"
        );
    }

    #[test]
    fn append_tx_jsonl() {
        let dir = std::env::temp_dir().join(format!(
            "pwm_tui_journal_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let entry = JournalEntry::pending(
            "send",
            "pwm1-to".to_string(),
            "1PWM".to_string(),
            "0PWM".to_string(),
            7,
        );

        append_tx(&dir, "pwm1-CY/7E-f00000000-tAA", &entry).expect("append tx");

        let raw = fs::read_to_string(dir.join("tx-history/pwm1-CY-7E-f00000000-tAA.jsonl"))
            .expect("read journal");
        assert!(raw.contains("\"kind\":\"send\""));
        assert!(raw.ends_with('\n'));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn read_journal_skips_bad() {
        let dir = std::env::temp_dir().join(format!(
            "pwm_tui_journal_read_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let entry = JournalEntry::pending(
            "burn",
            "pwm1-to".to_string(),
            "2PWM".to_string(),
            "0PWM".to_string(),
            8,
        );
        append_tx(&dir, "pwm1-CY/7E-f00000000-tBB", &entry).expect("append tx");
        fs::write(
            dir.join("tx-history/pwm1-CY-7E-f00000000-tBB.jsonl"),
            "not-json\n{\"v\":1,\"ts\":1,\"kind\":\"burn\",\"to\":\"pwm1-to\",\"amount_pwm\":\"2PWM\",\"fee_pwm\":\"0PWM\",\"nonce\":8,\"status\":\"ok\",\"execute_at_height\":null}\n",
        )
        .expect("write mixed journal");

        let rows = read_journal(&dir, "pwm1-CY/7E-f00000000-tBB");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "burn");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn status_update_entry() {
        let entry = JournalEntry::status_update(9);

        assert_eq!(entry.kind, "status_update");
        assert_eq!(entry.status, "ok");
        assert_eq!(entry.nonce, 9);
    }
}
