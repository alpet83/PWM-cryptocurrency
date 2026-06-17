//! Graceful node shutdown: persist snapshot then stop HTTP server.

use crate::snapshot::SealPersistMode;
use crate::state::InitPhase;
use crate::App;
use axum::extract::State;
use axum::http::StatusCode;
use std::path::Path;
use std::sync::atomic::Ordering;
use tracing::{error, info};

#[derive(Clone, Copy, Debug)]
pub(crate) enum ShutdownReason {
    Rpc,
    Signal(&'static str),
    DebugStop,
}

impl ShutdownReason {
    fn key(self) -> &'static str {
        match self {
            ShutdownReason::Rpc => "rpc",
            ShutdownReason::Signal(kind) => kind,
            ShutdownReason::DebugStop => "debug_stop",
        }
    }
}

pub(crate) async fn graceful_shutdown_request(
    app: &App,
    reason: ShutdownReason,
) -> Result<(), String> {
    if app.shutdown_requested.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let mut snapshot_err = None;
    if let Some(skip_reason) = shutdown_skip_reason(app).await {
        info!("shutdown snapshot persist skipped reason={skip_reason}");
    } else {
        let inner = app.inner.read().await;
        if let Some(ref backend) = app.autosnapshot_backend {
            if let Err(err) = backend.save_seal_persist(&inner, SealPersistMode::ShutdownFull) {
                snapshot_err = Some(format!("shutdown snapshot persist failed: {err}"));
            }
        }
    }
    if let Ok(mut slot) = app.shutdown_tx.lock() {
        if let Some(tx) = slot.take() {
            let _ = tx.send(());
        }
    }
    info!(
        "#INFO: pwmd остановлено оператором reason={} node_id={}",
        reason.key(),
        app.node_instance_id
    );
    info!(
        "#INFO: pwmd stopped by operator reason={} node_id={}",
        reason.key(),
        app.node_instance_id
    );
    if let Some(err) = snapshot_err {
        error!("{}", err);
        return Err(err);
    }
    Ok(())
}

async fn shutdown_skip_reason(app: &App) -> Option<&'static str> {
    let phase = {
        let st = app.init.read().await;
        st.phase
    };
    if matches!(phase, InitPhase::Starting | InitPhase::LoadingSnapshot) {
        return Some("loading_snapshot");
    }
    checkpoint_regress_skip(app).await
}

async fn checkpoint_regress_skip(app: &App) -> Option<&'static str> {
    let Some(ref backend) = app.autosnapshot_backend else {
        return None;
    };
    let Some(path) = backend.init_state_path() else {
        return None;
    };
    if would_regress_checkpoint(app, path.as_path())
        .await
        .unwrap_or(false)
    {
        return Some("checkpoint_regress");
    }
    None
}

async fn would_regress_checkpoint(app: &App, summary_path: &Path) -> Result<bool, String> {
    let Some(man) = crate::snapshot::incremental::read_epoch_manifest(summary_path)? else {
        return Ok(false);
    };
    let tip_h = {
        let inner = app.inner.read().await;
        inner.chain.tip_h()
    };
    Ok(tip_h < man.canonical_h)
}

pub(super) async fn v1_shutdown(
    State(app): State<App>,
) -> Result<StatusCode, (StatusCode, String)> {
    graceful_shutdown_request(&app, ShutdownReason::Rpc)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::{graceful_shutdown_request, ShutdownReason};
    use crate::app_from_dev_net;
    use crate::snapshot::{save_checkpoint_summary, SnapshotBackend};
    use crate::state::InitState;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn mk_temp_path(tag: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("pwmd-shutdown-{tag}-{ts}/pwm-data.json"))
    }

    #[tokio::test]
    async fn shutdown_request_sets_guard() {
        let app = app_from_dev_net();
        let _ = graceful_shutdown_request(&app, ShutdownReason::DebugStop).await;
        assert!(app.shutdown_requested.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn shutdown_skip_when_loading_snapshot() {
        let mut seed_app = app_from_dev_net();
        let snap_path = mk_temp_path("loading");
        let snap_dir = snap_path.parent().expect("summary parent").to_path_buf();
        fs::create_dir_all(&snap_dir).expect("create snapshot parent");
        seed_app.autosnapshot_backend = Some(SnapshotBackend::JsonFile {
            path: snap_path.clone(),
        });
        {
            let mut g = seed_app.inner.write().await;
            for _ in 0..5 {
                g.chain.seal(vec![]).expect("seal block");
            }
            save_checkpoint_summary(&snap_path, &g).expect("seed summary");
        }

        let mut app = app_from_dev_net();
        app.autosnapshot_backend = Some(SnapshotBackend::JsonFile {
            path: snap_path.clone(),
        });
        {
            let mut init = app.init.write().await;
            *init = InitState::loading(Some(snap_path.clone()));
        }
        let res = graceful_shutdown_request(&app, ShutdownReason::DebugStop).await;
        assert!(res.is_ok(), "shutdown should skip persist in loading state");

        let txt = fs::read_to_string(&snap_path).expect("read summary after shutdown");
        let raw: Value = serde_json::from_str(&txt).expect("parse summary json");
        let checkpoint_h = raw
            .get("checkpoint_height")
            .and_then(Value::as_u64)
            .expect("checkpoint_height field");
        assert_eq!(
            checkpoint_h, 5,
            "loading shutdown must not regress checkpoint"
        );
    }

    #[tokio::test]
    async fn shutdown_skip_checkpoint_regress() {
        let mut seed_app = app_from_dev_net();
        let snap_path = mk_temp_path("regress");
        let snap_dir = snap_path.parent().expect("summary parent").to_path_buf();
        fs::create_dir_all(&snap_dir).expect("create snapshot parent");
        seed_app.autosnapshot_backend = Some(SnapshotBackend::JsonFile {
            path: snap_path.clone(),
        });
        {
            let mut g = seed_app.inner.write().await;
            for _ in 0..5 {
                g.chain.seal(vec![]).expect("seal block");
            }
            save_checkpoint_summary(&snap_path, &g).expect("seed summary");
        }
        let manifest_path = snap_dir.join("epochs").join("pwm-epochs-manifest.json");
        let manifest_parent = manifest_path.parent().expect("manifest parent");
        fs::create_dir_all(manifest_parent).expect("create epochs dir");
        fs::write(
            &manifest_path,
            r#"{"schema_v":1,"epoch_span":1000,"canonical_h":124061,"tip_hash":"00","epochs":[]}"#,
        )
        .expect("write manifest");

        let mut app = app_from_dev_net();
        app.autosnapshot_backend = Some(SnapshotBackend::JsonFile {
            path: snap_path.clone(),
        });
        {
            let mut init = app.init.write().await;
            *init = InitState::ready(Some(snap_path.clone()));
        }

        let res = graceful_shutdown_request(&app, ShutdownReason::DebugStop).await;
        assert!(res.is_ok(), "checkpoint regress should skip persist");

        let txt = fs::read_to_string(&snap_path).expect("read summary after shutdown");
        let raw: Value = serde_json::from_str(&txt).expect("parse summary json");
        let checkpoint_h = raw
            .get("checkpoint_height")
            .and_then(Value::as_u64)
            .expect("checkpoint_height field");
        assert_eq!(checkpoint_h, 5, "summary checkpoint must remain unchanged");
    }
}
