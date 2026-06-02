//! Snapshot persistence (chain tip + state + roaming); facade for submodules.

// Re-exports are used from sibling modules and `#[cfg(test)]` prelude; `cargo check` lib-only
// does not compile test-only paths, so rustc may flag these as unused.
#![allow(unused_imports)]

mod anchor;
pub(crate) mod ch_http;
pub(crate) mod epoch;
mod genesis;
pub(crate) mod incremental;
mod io;
mod repair;
mod store;
pub(crate) mod telemetry;
mod types;

pub use genesis::load_genesis_bundle;
pub(crate) use genesis::snapshot_genesis_accounts;
pub(crate) use io::{
    decode_snap_raw, encode_inner_snap_json, encode_snap_data_txt, json_file_seal_persist,
    load_snapshot, load_snapshot_timed, replay_validate, save_checkpoint_summary,
    save_epochs_sum_tip, save_snapshot, snap_wire_json_bytes, SnapshotLoadOpts,
};
pub use repair::{repair_json_epochs, SnapRepairOpts, SnapRepairReport};
pub(crate) use store::{SealPersistMode, SnapshotBackend};
pub(crate) use telemetry::{SnapIoTiming, SNAP_STARTUP_TARGET};
pub(crate) use types::{BlocksStored, SnapshotData, SnapshotRoamingWire, SNAPSHOT_VERSION};
