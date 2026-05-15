//! Dev lane identifiers and runtime network/cluster identity for pwmd.

use serde::Deserialize;
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DevLane {
    #[default]
    Lane0,
    Lane1,
}

impl DevLane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lane0 => "lane0",
            Self::Lane1 => "lane1",
        }
    }
}

fn domain_namespace(domain_hi: u8) -> String {
    format!("domain-hi-0x{domain_hi:02x}")
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RuntimeIdentity {
    pub network_id: String,
    pub cluster_domain_hi: u8,
    pub cluster_id: String,
    pub node_id: String,
    pub mode: RuntimeIdentityMode,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RuntimeIdentityMode {
    /// Explicit domain config is provided; shard-enforced local tx guards are active.
    Explicit,
    /// Relay-compatible neutral baseline without shard alias affinity.
    Neutral,
}

impl RuntimeIdentityMode {
    pub fn is_shard_enforced(self) -> bool {
        matches!(self, Self::Explicit)
    }

    pub fn as_runtime_label(self) -> &'static str {
        match self {
            Self::Explicit => "shard_enforced",
            Self::Neutral => "relay_baseline",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeIdentityInput {
    pub network_id: Option<String>,
    pub cluster_domain_hi: Option<u8>,
    pub cluster_id: Option<String>,
    pub node_id: Option<String>,
}

impl RuntimeIdentityInput {
    fn missing_fields(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.network_id.is_none() {
            missing.push("network_id");
        }
        if self.cluster_domain_hi.is_none() {
            missing.push("cluster_domain_hi");
        }
        if self.cluster_id.is_none() {
            missing.push("cluster_id");
        }
        if self.node_id.is_none() {
            missing.push("node_id");
        }
        missing
    }

    fn has_any(&self) -> bool {
        self.network_id.is_some()
            || self.cluster_domain_hi.is_some()
            || self.cluster_id.is_some()
            || self.node_id.is_some()
    }
}

pub fn parse_cluster_domain_hi(raw: &str) -> Result<u8, String> {
    let trimmed = raw.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u8::from_str_radix(hex, 16)
            .map_err(|_| format!("invalid --cluster-domain-hi value {trimmed:?}; expected u8/hex"))
    } else {
        trimmed
            .parse::<u8>()
            .map_err(|_| format!("invalid --cluster-domain-hi value {trimmed:?}; expected u8/hex"))
    }
}

pub fn resolve_runtime_identity(
    dev_lane: DevLane,
    input: RuntimeIdentityInput,
) -> Result<RuntimeIdentity, String> {
    if !input.has_any() {
        return Ok(default_dev_lane_identity(dev_lane));
    }
    let missing = input.missing_fields();
    if !missing.is_empty() {
        return Err(format!(
            "partial identity configuration is not allowed; missing fields: {}",
            missing.join(", ")
        ));
    }
    Ok(RuntimeIdentity {
        network_id: input.network_id.expect("checked missing_fields"),
        cluster_domain_hi: input.cluster_domain_hi.expect("checked missing_fields"),
        cluster_id: input.cluster_id.expect("checked missing_fields"),
        node_id: input.node_id.expect("checked missing_fields"),
        mode: RuntimeIdentityMode::Explicit,
    })
}

/// Returns explicit devnet identity defaults for a given local dev lane.
pub(crate) fn default_dev_lane_identity(dev_lane: DevLane) -> RuntimeIdentity {
    RuntimeIdentity {
        network_id: "devnet".to_string(),
        cluster_domain_hi: match dev_lane {
            DevLane::Lane0 => 0x10,
            DevLane::Lane1 => 0x20,
        },
        cluster_id: match dev_lane {
            DevLane::Lane0 => "dev-cluster-0x10".to_string(),
            DevLane::Lane1 => "dev-cluster-0x20".to_string(),
        },
        node_id: match dev_lane {
            DevLane::Lane0 => "dev-node-0x10".to_string(),
            DevLane::Lane1 => "dev-node-0x20".to_string(),
        },
        mode: RuntimeIdentityMode::Explicit,
    }
}

pub fn default_runtime_identity_neutral() -> RuntimeIdentity {
    RuntimeIdentity {
        network_id: "devnet".to_string(),
        cluster_domain_hi: 0x00,
        cluster_id: "relay-neutral".to_string(),
        node_id: "relay-neutral".to_string(),
        mode: RuntimeIdentityMode::Neutral,
    }
}

pub fn storage_namespace(identity: &RuntimeIdentity) -> String {
    match identity.mode {
        RuntimeIdentityMode::Explicit => domain_namespace(identity.cluster_domain_hi),
        RuntimeIdentityMode::Neutral => "neutral".to_string(),
    }
}

/// Path segment under `state_root/neutral/` so two Neutral nodes do not share one `pwm-data.json`.
pub fn neutral_listen_dir_tag(listen: SocketAddr) -> String {
    listen.to_string().replace(':', "+")
}

pub fn runtime_shard_label(identity: &RuntimeIdentity, _dev_lane: DevLane) -> String {
    match identity.mode {
        RuntimeIdentityMode::Neutral => "neutral".to_string(),
        RuntimeIdentityMode::Explicit => {
            if let Some(entry) =
                pwm_core::domain_index::lookup_regulatory_by_hi(identity.cluster_domain_hi)
            {
                entry.label.to_string()
            } else {
                format!("0x{:02X}", identity.cluster_domain_hi)
            }
        }
    }
}
