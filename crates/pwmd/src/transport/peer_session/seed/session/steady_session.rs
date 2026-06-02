//! Heartbeat-driven steady peer loop after initial handshake merge.

use super::super::super::*;
use super::super::super::{
    handshake_write_traced, mark_trusted_peer_live, merge_account_views, merge_cross_shard_facts,
    peer_heartbeat_wire, peer_sync_v1, read_wire_msg, route_cluster_stub, route_sync_stub,
    send_account_views, send_cluster_prop, send_cross_shard_facts, send_sync_tx_batch,
    sticky_session_window_ms, sync_live, sync_mode_text, try_prop_nudge, write_wire_msg,
    PeerWireMsg,
};
use super::PostInitialExchange;
use crate::block_timing;
use crate::handshake::NodeHello;

#[derive(Clone, Copy)]
struct PeriodicTask {
    period_ms: u64,
    last_run_ms: u64,
}

impl PeriodicTask {
    fn new(period_ms: u64, anchor_ms: u64) -> Self {
        Self {
            period_ms: period_ms.max(1),
            last_run_ms: anchor_ms.saturating_sub(period_ms.max(1)),
        }
    }

    fn is_due(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_run_ms) >= self.period_ms
    }

    fn mark_ran(&mut self, now_ms: u64) {
        self.last_run_ms = now_ms;
    }

    fn until_due_ms(&self, now_ms: u64) -> u64 {
        let elapsed = now_ms.saturating_sub(self.last_run_ms);
        self.period_ms.saturating_sub(elapsed)
    }
}

struct SessionMicroScheduler {
    heartbeat: PeriodicTask,
    cluster_prop: PeriodicTask,
    cross_shard: PeriodicTask,
    account_views: PeriodicTask,
    sync_tx_batch: PeriodicTask,
    sync_tip: PeriodicTask,
}

#[derive(Default)]
struct FastLoopProbe {
    ticks: u64,
    rx_total: u64,
    rx_timeout_total: u64,
    rx_read_err_total: u64,
    rx_hb_ack_total: u64,
    rx_hb_total: u64,
    rx_cluster_total: u64,
    rx_sync_total: u64,
    rx_other_total: u64,
    rx_window: u64,
    timeout_window: u64,
    read_err_window: u64,
    hb_ack_window: u64,
    hb_window: u64,
    cluster_window: u64,
    sync_window: u64,
    other_window: u64,
    last_rx_tick: u64,
}

impl FastLoopProbe {
    fn on_rx(&mut self) {
        self.rx_total = self.rx_total.saturating_add(1);
        self.rx_window = self.rx_window.saturating_add(1);
        self.last_rx_tick = self.ticks;
    }

    fn log_and_reset_window(
        &mut self,
        seed: &std::net::SocketAddr,
        node_id: &str,
        role: ClusterRole,
    ) {
        info!(
            target: "pwmd::peer",
            "seed_fast_loop_probe seed={} node_id={} role={:?} ticks={} no_rx_ticks={} window_rx={} window_timeout={} window_read_err={} window_hb_ack={} window_hb={} window_cluster={} window_sync={} window_other={} total_rx={} total_timeout={} total_read_err={}",
            seed,
            node_id,
            role,
            self.ticks,
            self.ticks.saturating_sub(self.last_rx_tick),
            self.rx_window,
            self.timeout_window,
            self.read_err_window,
            self.hb_ack_window,
            self.hb_window,
            self.cluster_window,
            self.sync_window,
            self.other_window,
            self.rx_total,
            self.rx_timeout_total,
            self.rx_read_err_total,
        );
        self.rx_window = 0;
        self.timeout_window = 0;
        self.read_err_window = 0;
        self.hb_ack_window = 0;
        self.hb_window = 0;
        self.cluster_window = 0;
        self.sync_window = 0;
        self.other_window = 0;
    }
}

impl SessionMicroScheduler {
    fn new(hb_ms: u64, anchor_ms: u64) -> Self {
        Self {
            heartbeat: PeriodicTask::new(hb_ms, anchor_ms),
            cluster_prop: PeriodicTask::new(hb_ms, anchor_ms),
            cross_shard: PeriodicTask::new(hb_ms, anchor_ms),
            account_views: PeriodicTask::new(hb_ms, anchor_ms),
            sync_tx_batch: PeriodicTask::new(hb_ms, anchor_ms),
            sync_tip: PeriodicTask::new(hb_ms, anchor_ms),
        }
    }

    fn next_due_in_ms(&self, now_ms: u64) -> u64 {
        [
            self.heartbeat.until_due_ms(now_ms),
            self.cluster_prop.until_due_ms(now_ms),
            self.cross_shard.until_due_ms(now_ms),
            self.account_views.until_due_ms(now_ms),
            self.sync_tx_batch.until_due_ms(now_ms),
            self.sync_tip.until_due_ms(now_ms),
        ]
        .into_iter()
        .min()
        .unwrap_or(1)
        .max(1)
    }
}

pub(super) async fn run_seed_steady_session(
    app: &App,
    cfg: &TransportConfig,
    seed: std::net::SocketAddr,
    seed_key: &str,
    now_ms: u64,
    stream: &mut tokio::net::TcpStream,
    remote: NodeHello,
    drain_timeout_cap_ms: u64,
    post: PostInitialExchange,
) {
    const SEED_READ_POLL_MS: u64 = 50;
    let mut close_reason = post.close_reason;
    let mut close_detail = post.close_detail;
    let hb = cfg.heartbeat_interval_ms.max(200);
    let mut live = close_reason.is_none();
    let sync_v1 = peer_sync_v1(&remote);
    let same_shard = remote.cluster.domain_hi == app.identity.cluster_domain_hi;
    let (sync_hdr_cap, sync_blk_cap) = sync_live::sync_caps(&remote);
    let can_cup = remote
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.supports_epoch_catchup)
        .unwrap_or(false);
    let mut sync_seq_no = 0u64;
    let mut sched = SessionMicroScheduler::new(hb, current_time_ms().unwrap_or(now_ms));
    let mut probe = FastLoopProbe::default();
    info!(
        target: "pwmd::peer",
        "peer sync mode negotiated seed={} node_id={} mode={}",
        seed,
        remote.node.node_id,
        sync_mode_text(&remote)
    );
    while live {
        probe.ticks = probe.ticks.saturating_add(1);
        let tick_open_ms = current_time_ms().unwrap_or(now_ms);
        if let Err(err) = try_prop_nudge(&app, &cfg, stream, &remote).await {
            let mut hs = handshake_write_traced(app, "seed_steady_session").await;
            close_reason = Some(wire_close_reason(&err));
            close_detail = detail_with_err("cluster_propose_ahead_write_failed", &err);
            set_peer_error(
                &mut hs,
                current_time_ms().unwrap_or(0),
                format!("seed {seed} cluster_propose_ahead_write_failed: {err}"),
            );
            break;
        }
        let ts = current_time_ms().unwrap_or(tick_open_ms);
        if sched.heartbeat.is_due(ts) {
            let hb_out = peer_heartbeat_wire(&app, ts).await;
            if let Err(err) = write_wire_msg(stream, &hb_out, cfg.heartbeat_timeout_ms).await {
                let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("heartbeat_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    ts,
                    format!("seed {seed} wire_heartbeat_write_failed: {err}"),
                );
                break;
            }
            sched.heartbeat.mark_ran(ts);
        }
        if sched.cluster_prop.is_due(ts) {
            if let Err(err) = send_cluster_prop(&app, &cfg, stream, &remote).await {
                let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("cluster_propose_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    ts,
                    format!("seed {seed} cluster_propose_write_failed: {err}"),
                );
                break;
            }
            sched.cluster_prop.mark_ran(ts);
        }
        if sched.cross_shard.is_due(ts) {
            if let Err(err) = send_cross_shard_facts(&app, &cfg, stream).await {
                let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("cross_shard_facts_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    ts,
                    format!("seed {seed} cross_shard_facts_write_failed: {err}"),
                );
                break;
            }
            sched.cross_shard.mark_ran(ts);
        }
        if sched.account_views.is_due(ts) {
            if let Err(err) = send_account_views(&app, &cfg, stream).await {
                let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("account_views_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    ts,
                    format!("seed {seed} account_views_write_failed: {err}"),
                );
                break;
            }
            sched.account_views.mark_ran(ts);
        }
        if sched.sync_tx_batch.is_due(ts) {
            if let Err(err) =
                send_sync_tx_batch(&app, &cfg, stream, &remote, &mut sync_seq_no).await
            {
                let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("sync_tx_batch_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    ts,
                    format!("seed {seed} sync_tx_batch_write_failed: {err}"),
                );
                break;
            }
            sched.sync_tx_batch.mark_ran(ts);
        }
        if sched.sync_tip.is_due(ts) {
            if let Err(err) =
                sync_live::send_sync_tip(&app, &cfg, stream, &remote, &mut sync_seq_no).await
            {
                let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                close_reason = Some(wire_close_reason(&err));
                close_detail = detail_with_err("sync_tip_write_failed", &err);
                set_peer_error(
                    &mut hs,
                    ts,
                    format!("seed {seed} sync_tip_write_failed: {err}"),
                );
                break;
            }
            sched.sync_tip.mark_ran(ts);
        }
        let mut heartbeat_acked = false;
        let mut read_timeout_ms = cfg
            .heartbeat_timeout_ms
            .min(sched.next_due_in_ms(current_time_ms().unwrap_or(ts)));
        let mut read_arm_ms = current_time_ms().unwrap_or(ts);
        let mut read_arm_set = false;
        while live {
            if !read_arm_set {
                read_arm_ms = current_time_ms().unwrap_or(ts);
                read_arm_set = true;
            }
            let poll_ms = read_timeout_ms.min(SEED_READ_POLL_MS).max(1);
            match tokio::time::timeout(std::time::Duration::from_millis(poll_ms), stream.readable())
                .await
            {
                Err(_) => {
                    probe.rx_timeout_total = probe.rx_timeout_total.saturating_add(1);
                    probe.timeout_window = probe.timeout_window.saturating_add(1);
                    break;
                }
                Ok(Err(err)) => {
                    probe.rx_read_err_total = probe.rx_read_err_total.saturating_add(1);
                    probe.read_err_window = probe.read_err_window.saturating_add(1);
                    let err = format!("wire_readable_failed: {err}");
                    let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                    let reason = wire_close_reason(&err);
                    close_reason = Some(reason);
                    close_detail = detail_with_err("heartbeat_readable_failed", &err);
                    set_peer_error(
                        &mut hs,
                        ts,
                        format!("seed {seed} wire_heartbeat_readable_failed: {err}"),
                    );
                    live = false;
                    continue;
                }
                Ok(Ok(())) => {}
            }
            match read_wire_msg(stream, read_timeout_ms).await {
                Ok(PeerWireMsg::HeartbeatAck { .. }) => {
                    probe.on_rx();
                    probe.rx_hb_ack_total = probe.rx_hb_ack_total.saturating_add(1);
                    probe.hb_ack_window = probe.hb_ack_window.saturating_add(1);
                    heartbeat_acked = true;
                    let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                }
                Ok(PeerWireMsg::Heartbeat {
                    unix_ms,
                    chain_tip_height,
                    federation_shard_id,
                    federation_gossip,
                    ..
                }) => {
                    probe.on_rx();
                    probe.rx_hb_total = probe.rx_hb_total.saturating_add(1);
                    probe.hb_window = probe.hb_window.saturating_add(1);
                    crate::federation::merge_remote_hb(
                        &app,
                        true,
                        &remote,
                        &remote.node.node_id,
                        unix_ms,
                        chain_tip_height,
                        federation_shard_id,
                        federation_gossip,
                    )
                    .await;
                    if let Err(err) = write_wire_msg(
                        stream,
                        &PeerWireMsg::HeartbeatAck { unix_ms },
                        cfg.heartbeat_timeout_ms,
                    )
                    .await
                    {
                        let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                        close_reason = Some(wire_close_reason(&err));
                        close_detail = detail_with_err("heartbeat_ack_write_failed", &err);
                        set_peer_error(
                            &mut hs,
                            ts,
                            format!("seed {seed} wire_heartbeat_ack_write_failed: {err}"),
                        );
                        live = false;
                    } else {
                        let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                        mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                    }
                }
                Ok(PeerWireMsg::CrossShardFacts { facts }) => {
                    probe.on_rx();
                    probe.rx_other_total = probe.rx_other_total.saturating_add(1);
                    probe.other_window = probe.other_window.saturating_add(1);
                    merge_cross_shard_facts(&app, facts, true).await;
                    let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                }
                Ok(PeerWireMsg::AccountViews { rows }) => {
                    probe.on_rx();
                    probe.rx_other_total = probe.rx_other_total.saturating_add(1);
                    probe.other_window = probe.other_window.saturating_add(1);
                    merge_account_views(
                        &app,
                        rows,
                        true,
                        &remote.node.node_id,
                        remote.cluster.domain_hi,
                        ts,
                    )
                    .await;
                    let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                }
                Ok(
                    msg @ (PeerWireMsg::ClusterPropose { .. } | PeerWireMsg::ClusterAttest { .. }),
                ) => {
                    probe.on_rx();
                    probe.rx_cluster_total = probe.rx_cluster_total.saturating_add(1);
                    probe.cluster_window = probe.cluster_window.saturating_add(1);
                    let maybe_prop = match &msg {
                        PeerWireMsg::ClusterPropose { msg } => Some((msg.height, msg.round)),
                        _ => None,
                    };
                    if let Some((h, r)) = maybe_prop {
                        let recv_ms = current_time_ms().unwrap_or(ts);
                        let tick_sleep_ms = ts.saturating_sub(tick_open_ms);
                        let pre_read_ms = read_arm_ms.saturating_sub(ts);
                        let read_wait_ms = recv_ms.saturating_sub(read_arm_ms);
                        let since_tick_ms = recv_ms.saturating_sub(tick_open_ms);
                        // Focused hypothesis probe: if this is large, attester wake/read cadence,
                        // not attest processing itself, is likely dominating proposer-side wall time.
                        let severe = read_wait_ms > hb.saturating_mul(2)
                            || since_tick_ms > hb.saturating_mul(3)
                            || pre_read_ms > hb;
                        if severe || h % 50 == 0 {
                            let level = if severe { "WARN" } else { "INFO" };
                            if severe {
                                warn!(
                                    target: "pwmd::peer",
                                    "attester_cluster_rx_timing level={} node_id={} h={} r={} tick_sleep_ms={} pre_read_ms={} read_wait_ms={} since_tick_ms={} hb_ms={} timeout_ms={}",
                                    level,
                                    remote.node.node_id,
                                    h,
                                    r,
                                    tick_sleep_ms,
                                    pre_read_ms,
                                    read_wait_ms,
                                    since_tick_ms,
                                    hb,
                                    cfg.heartbeat_timeout_ms,
                                );
                            } else {
                                info!(
                                    target: "pwmd::peer",
                                    "attester_cluster_rx_timing level={} node_id={} h={} r={} tick_sleep_ms={} pre_read_ms={} read_wait_ms={} since_tick_ms={} hb_ms={} timeout_ms={}",
                                    level,
                                    remote.node.node_id,
                                    h,
                                    r,
                                    tick_sleep_ms,
                                    pre_read_ms,
                                    read_wait_ms,
                                    since_tick_ms,
                                    hb,
                                    cfg.heartbeat_timeout_ms,
                                );
                            }
                        }
                    }
                    let route_cluster_start_at = std::time::Instant::now();
                    let route_cluster_start_ms = current_time_ms().unwrap_or(ts);
                    let maybe_attest = route_cluster_stub(app, &remote.node.node_id, msg).await;
                    let route_cluster_done_ms = current_time_ms().unwrap_or(route_cluster_start_ms);
                    let route_cluster_latency_ms =
                        (route_cluster_start_at.elapsed().as_micros() as f64) / 1000.0;
                    if route_cluster_latency_ms >= 100.0 {
                        warn!(
                            target: "pwmd::peer",
                            "seed cluster_route_slow seed={} node_id={} ts_ms={} latency_ms={:.2}",
                            seed,
                            remote.node.node_id,
                            route_cluster_done_ms,
                            route_cluster_latency_ms,
                        );
                    }
                    if let Some(attest) = maybe_attest {
                        let h = attest.height;
                        let r = attest.round;
                        if let Err(err) = write_wire_msg(
                            stream,
                            &PeerWireMsg::ClusterAttest { msg: attest },
                            cfg.heartbeat_timeout_ms,
                        )
                        .await
                        {
                            let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                            close_reason = Some(wire_close_reason(&err));
                            close_detail = detail_with_err("cluster_attest_write_failed", &err);
                            set_peer_error(
                                &mut hs,
                                ts,
                                format!("seed {seed} cluster_attest_write_failed: {err}"),
                            );
                            live = false;
                            continue;
                        }
                        if let Some(bt) = app.block_timing.as_ref() {
                            block_timing::note_att_wire(
                                bt,
                                block_timing::AttCtx {
                                    h,
                                    r,
                                    t_ms: block_timing::now_ms_f64(),
                                    att_id: app.node_instance_id.clone(),
                                },
                            );
                        }
                    }
                    let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                    mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                }
                Ok(
                    msg @ (PeerWireMsg::SyncProfileAnnounce { .. }
                    | PeerWireMsg::SyncTipAnnounce { .. }
                    | PeerWireMsg::SyncHeadersReq { .. }
                    | PeerWireMsg::SyncHeadersBatch { .. }
                    | PeerWireMsg::SyncBlocksReq { .. }
                    | PeerWireMsg::SyncBlocksBatch { .. }
                    | PeerWireMsg::SyncTxAnnounce { .. }
                    | PeerWireMsg::SyncTxReq { .. }
                    | PeerWireMsg::SyncTxBatch { .. }
                    | PeerWireMsg::SyncNack { .. }
                    | PeerWireMsg::SyncCatchupReq { .. }
                    | PeerWireMsg::SyncCatchupChunk { .. }
                    | PeerWireMsg::SyncCatchupDone { .. }),
                ) => {
                    probe.on_rx();
                    probe.rx_sync_total = probe.rx_sync_total.saturating_add(1);
                    probe.sync_window = probe.sync_window.saturating_add(1);
                    let outcome = route_sync_stub(
                        app,
                        cfg,
                        stream,
                        Some(seed_key),
                        &remote.node.node_id,
                        msg,
                        sync_v1,
                        app.identity.cluster_domain_hi,
                        same_shard,
                        sync_hdr_cap,
                        sync_blk_cap,
                        can_cup,
                        &mut sync_seq_no,
                    )
                    .await;
                    match outcome {
                        super::super::super::SyncRouteOutcome::Continue => {
                            let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                            mark_trusted_peer_live(&mut hs, &remote.node.node_id, ts);
                        }
                        super::super::super::SyncRouteOutcome::Disconnect { reason, detail } => {
                            close_reason = Some(reason);
                            close_detail = detail;
                            live = false;
                        }
                    }
                }
                Ok(other) => {
                    probe.on_rx();
                    probe.rx_other_total = probe.rx_other_total.saturating_add(1);
                    probe.other_window = probe.other_window.saturating_add(1);
                    let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                    close_reason = Some(PeerCloseReason::ProtocolError);
                    close_detail = format!("heartbeat_unexpected: {other:?}");
                    set_peer_error(
                        &mut hs,
                        ts,
                        format!("seed {seed} wire_heartbeat_unexpected: {other:?}"),
                    );
                    live = false;
                }
                Err(err) => {
                    if is_wire_timeout(&err) {
                        probe.rx_timeout_total = probe.rx_timeout_total.saturating_add(1);
                        probe.timeout_window = probe.timeout_window.saturating_add(1);
                        break;
                    }
                    probe.rx_read_err_total = probe.rx_read_err_total.saturating_add(1);
                    probe.read_err_window = probe.read_err_window.saturating_add(1);
                    let mut hs = handshake_write_traced(app, "seed_steady_session").await;
                    let reason = wire_close_reason(&err);
                    close_reason = Some(reason);
                    close_detail = detail_with_err("heartbeat_read_failed", &err);
                    set_peer_error(
                        &mut hs,
                        ts,
                        format!("seed {seed} wire_heartbeat_read_failed: {err}"),
                    );
                    live = false;
                }
            }
            if !live || !heartbeat_acked {
                continue;
            }
            read_timeout_ms = cfg.heartbeat_timeout_ms.min(drain_timeout_cap_ms).max(1);
        }
        if probe.ticks % 100 == 0 {
            probe.log_and_reset_window(&seed, &remote.node.node_id, app.cluster_cfg.role);
        }
    }
    {
        let mut hs = handshake_write_traced(app, "seed_steady_session").await;
        let reason = close_reason.unwrap_or(PeerCloseReason::ExplicitShutdown);
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            seed_key,
            Some(&remote.node.node_id),
            reason,
            Some(close_detail.as_str()),
        );
        record_reconnect(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            seed_key,
            reconnect_from_close(reason),
            Some(close_detail.as_str()),
        );
        let sticky_window_ms = sticky_session_window_ms(cfg);
        let disconnect_ts = current_time_ms().unwrap_or(0);
        if let Some(last_node_id) = hs
            .transport
            .seed_peers
            .get(seed_key)
            .and_then(|s| s.last_node_id.clone())
        {
            if let Some(peer) = hs.peers.get_mut(&last_node_id) {
                let recently_alive =
                    disconnect_ts.saturating_sub(peer.last_seen_ms) <= sticky_window_ms;
                if !recently_alive {
                    peer.status = PeerStatus::Disconnected;
                }
            }
        }
    }
}
