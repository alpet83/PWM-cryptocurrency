//! RPC source IP allowlist parsing, matching, and middleware.

use crate::App;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub(crate) struct RpcCidr {
    net: IpAddr,
    prefix: u8,
}

#[derive(Clone)]
pub(crate) struct RpcAllowState {
    cidrs: Arc<Vec<RpcCidr>>,
    dynamic: Arc<RwLock<HashSet<IpAddr>>>,
    auto_until: Option<Instant>,
}

impl Default for RpcAllowState {
    fn default() -> Self {
        Self {
            cidrs: Arc::new(Vec::new()),
            dynamic: Arc::new(RwLock::new(HashSet::new())),
            auto_until: None,
        }
    }
}

impl RpcAllowState {
    pub(crate) fn from_cfg(raw: &[String], auto_secs: u16) -> Result<Self, String> {
        let cidrs = raw
            .iter()
            .filter(|s| !s.trim().is_empty())
            .map(|s| parse_rpc_cidr(s.trim()))
            .collect::<Result<Vec<_>, _>>()?;
        let auto_until = if auto_secs == 0 {
            None
        } else {
            Some(Instant::now() + Duration::from_secs(u64::from(auto_secs)))
        };
        Ok(Self {
            cidrs: Arc::new(cidrs),
            dynamic: Arc::new(RwLock::new(HashSet::new())),
            auto_until,
        })
    }

    pub(crate) fn disabled(&self) -> bool {
        self.cidrs.is_empty() && self.auto_until.is_none()
    }

    pub(crate) async fn ip_allowed(&self, src: IpAddr) -> bool {
        if self.disabled() || self.cidrs.iter().any(|cidr| ip_in_cidr(src, cidr)) {
            return true;
        }
        if self.dynamic.read().await.contains(&src) {
            return true;
        }
        if self.auto_until.is_some_and(|until| Instant::now() < until) {
            let mut dynamic = self.dynamic.write().await;
            if dynamic.insert(src) {
                info!(%src, "rpc IP auto-enrolled");
            }
            return true;
        }
        false
    }

    #[cfg(test)]
    pub(crate) fn with_closed_auto(&self) -> Self {
        Self {
            cidrs: self.cidrs.clone(),
            dynamic: self.dynamic.clone(),
            auto_until: Some(Instant::now() - Duration::from_secs(1)),
        }
    }
}

pub(crate) async fn rpc_ip_gate(
    State(app): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    let Some(ConnectInfo(peer)) = conn else {
        if app.rpc_allow.disabled() {
            return Ok(next.run(req).await);
        }
        warn!("rpc request rejected: missing peer address");
        return Err((StatusCode::FORBIDDEN, "IP not in allowlist".to_string()));
    };
    let src = peer.ip();
    if app.rpc_allow.ip_allowed(src).await {
        Ok(next.run(req).await)
    } else {
        warn!(%src, "rpc request rejected by IP allowlist");
        Err((StatusCode::FORBIDDEN, "IP not in allowlist".to_string()))
    }
}

fn parse_rpc_cidr(raw: &str) -> Result<RpcCidr, String> {
    let (ip_raw, prefix_raw) = raw
        .split_once('/')
        .map_or((raw, None), |(ip, p)| (ip, Some(p)));
    let net = ip_raw
        .parse::<IpAddr>()
        .map_err(|e| format!("invalid rpc_allowed_ips entry {raw:?}: {e}"))?;
    let max = if net.is_ipv4() { 32 } else { 128 };
    let prefix = match prefix_raw {
        Some(p) => p
            .parse::<u8>()
            .map_err(|e| format!("invalid rpc_allowed_ips prefix in {raw:?}: {e}"))?,
        None => max,
    };
    if prefix > max {
        return Err(format!(
            "invalid rpc_allowed_ips prefix in {raw:?}: {prefix} exceeds {max}"
        ));
    }
    Ok(RpcCidr { net, prefix })
}

fn ip_in_cidr(src: IpAddr, cidr: &RpcCidr) -> bool {
    match (src, cidr.net) {
        (IpAddr::V4(a), IpAddr::V4(b)) => prefix_match(&a.octets(), &b.octets(), cidr.prefix),
        (IpAddr::V6(a), IpAddr::V6(b)) => prefix_match(&a.octets(), &b.octets(), cidr.prefix),
        _ => false,
    }
}

fn prefix_match(a: &[u8], b: &[u8], prefix: u8) -> bool {
    let full = usize::from(prefix / 8);
    let rem = prefix % 8;
    if a[..full] != b[..full] {
        return false;
    }
    if rem == 0 {
        return true;
    }
    let mask = u8::MAX << (8 - rem);
    (a[full] & mask) == (b[full] & mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_rpc_cidr ---

    #[test]
    fn bare_ipv4_gets_slash32() {
        let c = parse_rpc_cidr("1.2.3.4").unwrap();
        assert!(matches!(c.net, IpAddr::V4(_)));
        assert_eq!(c.prefix, 32);
    }

    #[test]
    fn bare_ipv6_gets_slash128() {
        let c = parse_rpc_cidr("2001:db8::1").unwrap();
        assert!(matches!(c.net, IpAddr::V6(_)));
        assert_eq!(c.prefix, 128);
    }

    #[test]
    fn cidr_notation_parsed() {
        let c = parse_rpc_cidr("10.0.0.0/8").unwrap();
        assert_eq!(c.prefix, 8);
    }

    #[test]
    fn invalid_ip_errors() {
        assert!(parse_rpc_cidr("not_an_ip").is_err());
    }

    #[test]
    fn prefix_too_large_errors() {
        assert!(parse_rpc_cidr("1.2.3.4/33").is_err());
        assert!(parse_rpc_cidr("::1/129").is_err());
    }

    // --- RpcAllowState::ip_allowed ---

    #[tokio::test]
    async fn ip_in_cidr_allowed() {
        let state = RpcAllowState::from_cfg(&["10.0.0.0/8".to_string()], 0).unwrap();
        assert!(state.ip_allowed("10.1.2.3".parse().unwrap()).await);
    }

    #[tokio::test]
    async fn ip_not_in_cidr_denied() {
        let state = RpcAllowState::from_cfg(&["10.0.0.0/8".to_string()], 0).unwrap();
        assert!(!state.ip_allowed("192.168.1.1".parse().unwrap()).await);
    }

    #[tokio::test]
    async fn disabled_state_allows_all() {
        let state = RpcAllowState::default();
        assert!(state.disabled());
        assert!(state.ip_allowed("9.9.9.9".parse().unwrap()).await);
    }

    #[tokio::test]
    async fn auto_enroll_during_window() {
        let state = RpcAllowState::from_cfg(&[], 60).unwrap();
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        assert!(state.ip_allowed(ip).await);
    }

    #[tokio::test]
    async fn auto_enroll_sticky_after_window_closes() {
        let state = RpcAllowState::from_cfg(&[], 60).unwrap();
        let ip: IpAddr = "5.6.7.8".parse().unwrap();
        // enroll during open window
        assert!(state.ip_allowed(ip).await);
        // simulate window expiry
        let closed = state.with_closed_auto();
        // still allowed via dynamic set
        assert!(closed.ip_allowed(ip).await);
    }

    #[tokio::test]
    async fn new_ip_blocked_after_window_closes() {
        let state = RpcAllowState::from_cfg(&[], 60).unwrap();
        let closed = state.with_closed_auto();
        let ip: IpAddr = "9.9.9.9".parse().unwrap();
        assert!(!closed.ip_allowed(ip).await);
    }
}
