# Changelog

Notable behavior and documentation changes. Section timestamps are **UTC**, derived from the authoring commit (`git show -s --format=%cI <hash>`). Parenthetical `abbrev` is the tip commit for that batch.

---

## 2026-05-04T07:44:46Z (`85bec28`)

### Added / changed

- **Mempool / seal:** `POST /v1/tx` runs tip `precheck_apply_tip` before enqueue; underfunded txs return **409** without entering the pool. **Seal loop** drops the first failing tx on apply errors instead of infinitely requeuing the same batch.
- **Bridge federation:** bridge-only `BridgeFederationCommitment` digest on compatible peers; `bridge_federation_trust` / `bridge_refusal_reason` on `/v1/status`; `POST /v1/bridge-federation/reset`; relay and peer hellos carry commitment where applicable.
- **Debug:** `--broke-trust-test` advertises a fake genesis digest in transport `NodeHello` so honest peers reject handshakes (operator negative testing).
- **Docs/tests:** tester guide updates; HTTP tests for underfunded transfer and export-readiness when bridge trust is latched; slice20 e2e coverage updates; operator/review notes.

---

## 2026-05-04T04:43:30Z (`38dcdc4`)

### Fixed

- **Cross-shard send from pwm-tui:** relayed `POST /v1/tx` (`Import`) could return HTTP **502** on the source and **400** on the peer (`invalid import: export_id is not known and embedded provenance is missing`), while the **source balance still decreased** once `Export` was sealed — no credit on the target. **Cause:** `pwm-tui` built a bare signed `Import` without `import_provenance`, unlike `pwm tx-import`, so `enforce_import_provenance_prefilter` on the recipient rejected the relay. **Fix:** before submitting the relay, fetch matching rows from **`GET /v1/cross-shard/facts`** on the **target** RPC (with backoff), build `ExportProvenance`, **`set_import_provenance_signed`** — same contract as **`pwm-cli`**. Extended retry backoff for transient `embedded provenance` messages.

---

## 2026-05-03 — cross-shard stabilization & snapshot stack

Batch on **2026-05-03** (commits through `b979153`; intermediate `chore(tasks)` traceability-only commits omitted here).

### Added / changed

- **JsonFile runtime save:** epoch persistence without monolithic full-epoch encode on each seal (`2212fbd`).
- **Snapshot diagnostics / repair:** replay mismatch diagnostics (`61fa3d4`); offline snapshot repair tool (`669a41a`).
- **Cross-shard:** import provenance replayable on target (`1270b06`); `GET`-style cross-shard backfill endpoint (`d56a699`); stabilization contract docs (`353d814`).
- **ClickHouse / incremental (slice7 wave4):** DDL alignment, `shard_balance`, validators table cfg (`551ce84`).
- **Docs:** sprint-15 closeout gate (`0584c84`), architecture review (`63a4200`), testing preflight scripts (`7d37f8d`, `bb9856d`), CH JsonFile fallback design (`f3a2d12`).

---

## Archive — since MVP multi-sprint plan (anchor `10b0b47`)

**Anchor:** `10b0b47` *feat(wallet,docs): Sprint 14 slices 1–3 and orchestrator guardrails* — **2026-04-28T07:06:33Z** — introduces `docs/plans/mvp_v1_testnet_multi-sprint.md` and related governance. Everything below summarizes **284** commits from `10b0b47~1..HEAD` (through **2026-05-04**); pure `chore(tasks)` / traceability-only SHAs are omitted in prose.

### 2026-05-02 (UTC day) — Sprint 15 **Slice-O.1** modularization waves

- **pwmd:** incremental decomposition of the transport stack into focused modules (metrics, tick, dial, lifecycle, spawn; `peer_session` → wire / inbound / seed with connect–handshake–session; `handshake_state`, `incoming_hello`; transport and seed-session test trees).
- **pwm-cli:** `main.rs` split into `cli_config`, `rpc_helpers`, `cli_cmd`, `cmd_*`, `wallet/`, `cli_parse`, `signer`, subprocess integration tests, and related docs closeouts (waves 5–18 narrative in commit subjects).
- **pwm-tui:** S15-O.1 waves 5–12 — extract models, status, config, modals, roaming/send_form/history, layout/footer, term.draw panels, `test_support`, narrow crate-root `pub`.
- **Meta:** `//!` module banners; test function names ≤5 segments; reviewer/orchestrator checklist traceability docs.

### 2026-05-01 — Cross-shard behavior, relay wiring, S15-O cleanup

- **pwmd:** federation table + gossip-style relay path; relay HTTP uses **RPC port − 100** vs peer convention (`ab9f9ad`); mirror roaming after relay import (`01b57dc`); cross-shard observability (handoff register, relay flow ids); identifier shortening after style review.
- **pwm-tui:** cross-shard Import after `relayed` + step-5 target balance; shared `TextInput` for modals / SendForm.
- **Docs / hygiene:** Sprint **3.17** roaming closeout & xshard doc sync (`5672fdd`); rename `ROUMING*` → `ROAMING*` (`6204dc2`); S15-O group A cleanup (`1b6c5a0`: TUI xflow, dial, deprecated `--shard` note).
- **Core / UI:** S15-O-B display, `wallet_io`, RPC helpers (`0ac777c`).

### 2026-04-30 — Stateful peer transport & operator validation

- Stateful **peer listener** transport (`4458c6a`); HTTP peer-seed handshake diagnostics (`105e401`); sprint-15 live connectivity / import-balance review captures.

### 2026-04-29 — Sprint 15 architecture track & one-window relay

- Export **readiness** preflight (`0afc8f9`); **foreign balance** semantics split (`98cf1b2`); genesis **join guardrails** (`91cb84a`); **trusted relay** for one-window cross-shard (`678fe82`); TUI staged cross-shard diagnostics (`1d550c9`, `042abc8`).
- Planning: sprint-15 architecture checklist (`4684517`); slice reviews (genesis consistency, import visibility).

### 2026-04-28 — Sprint 14 tail + plan landing

- **`10b0b47`:** multi-sprint plan + orchestrator rules on disk.
- Same & adjacent commits: Sprint 14 slices **4–11** / genesis / cluster / hardening (`f43550a` … `065c5f2`); logging fixes (`1d1fb9c`, …); `data_file` wiring for snapshots (`ca9df3e`).

---

### Machine-readable full history

```bash
git log 10b0b471e96f4f24f8c4e02074023701e588cdba~1..HEAD --reverse --format="%cI %h %s"
```

Use this for exact ordering, authors, and subjects; the narrative sections above are **grouped by calendar period** for readability only.
