//! Criterion harness: snapshot load via JsonFile vs ClickHouse (`clickhouse-snapshot`).
//! Prefers `./tmp/state-testnet/pwm-data.json` + `./tmp/genesis-custom.json` when present (see `node-1.ps1`).
//! Set `PWM_SNAPSHOT_BENCH_DIR` to a directory that contains `pwm-data.json` (e.g. full `tmp/state-testnet` tree).

use criterion::{black_box, Criterion};
use pwmd::snap_bench_hlp::BenchSnapCtx;

fn json_bench(c: &mut Criterion, ctx: &BenchSnapCtx) {
    let cfg = ctx.cfg.clone();
    let path = ctx.path.clone();
    if ctx.from_node_scripts {
        eprintln!(
            "snap_load_jsonfile: using node script snapshot {}",
            path.display()
        );
    }
    c.bench_function("snap_load_jsonfile", |b| {
        b.iter(|| {
            let w = pwmd::snap_bench_hlp::load_jsonfile_wire(
                black_box(path.as_path()),
                black_box(&cfg),
            )
            .unwrap();
            black_box(w);
        });
    });
}

/// Decode JSON **without** full replay vs isolate replay — production load does both (`load_snapshot`).
fn replay_breakdown_bench(c: &mut Criterion, ctx: &BenchSnapCtx) {
    let cfg = ctx.cfg.clone();
    let path = ctx.path.clone();
    let parsed = pwmd::snap_bench_hlp::SnapBenchParsed::decode_raw(path.as_path(), &cfg)
        .expect("bench decode raw");
    c.bench_function("snap_decode_trust_state", |b| {
        b.iter(|| {
            let hold = pwmd::snap_bench_hlp::SnapBenchParsed::decode_raw(
                black_box(path.as_path()),
                black_box(&cfg),
            )
            .unwrap();
            black_box(hold.wire_bytes().unwrap());
        });
    });
    c.bench_function("snap_validate_full_replay", |b| {
        b.iter(|| {
            parsed.replay_validate(black_box(&cfg)).unwrap();
        });
    });
}

#[cfg(feature = "clickhouse-snapshot")]
fn ch_bench(c: &mut Criterion, ctx: &BenchSnapCtx) {
    let cfg = ctx.cfg.clone();
    let path = ctx.path.clone();
    let want = pwmd::snap_bench_hlp::load_jsonfile_wire(&path, &cfg).expect("ref wire");
    let raw_for_mock = std::fs::read_to_string(&path).expect("snap file read");

    let use_live = std::env::var("PWM_CLICKHOUSE_BENCH_URL").is_ok();
    let base = if let Ok(u) = std::env::var("PWM_CLICKHOUSE_BENCH_URL") {
        u.trim().trim_end_matches('/').to_string()
    } else {
        pwmd::snap_bench_hlp::spawn_ch_json_mock(&cfg, raw_for_mock.as_str())
    };
    let ch = pwmd::snap_bench_hlp::mk_bench_ch_cfg(&base);
    if use_live {
        if let Err(e) = ch.import_snapshot_file(path.as_path(), &cfg) {
            eprintln!("snap_load_clickhouse: live CH import failed: {e}");
            return;
        }
    }
    let got = match pwmd::snap_bench_hlp::load_ch_wire(&ch, &cfg) {
        Ok(g) => g,
        Err(e) => {
            if use_live {
                eprintln!("snap_load_clickhouse: skip (live CH: {e})");
            } else {
                panic!("mock CH: {e}");
            }
            return;
        }
    };
    if got != want {
        if use_live {
            eprintln!("snap_load_clickhouse: v2 wire differs from JsonFile ref; bench still runs");
        } else {
            panic!("mock CH wire mismatch");
        }
    }
    c.bench_function("snap_load_clickhouse", |b| {
        b.iter(|| {
            let w = pwmd::snap_bench_hlp::load_ch_wire(black_box(&ch), black_box(&cfg)).unwrap();
            black_box(w);
        });
    });
}

pub fn main() {
    let mut c = Criterion::default().configure_from_args();
    let ctx = pwmd::snap_bench_hlp::resolve_bench_snapshot();
    json_bench(&mut c, &ctx);
    replay_breakdown_bench(&mut c, &ctx);
    #[cfg(feature = "clickhouse-snapshot")]
    ch_bench(&mut c, &ctx);
    c.final_summary();
    if !ctx.from_node_scripts {
        let _ = std::fs::remove_file(&ctx.path);
    }
}
