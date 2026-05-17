# Demo publication readiness review — 2026-05-17

**Repo slice:** documentation + checklist traceability for presenting the current snapshot as a **public-facing demo** (no `crates/` review in this gate).

**Verdict:** **PASS_WITH_NITS** (nits classified below; all **mechanical**, suitable for orchestrator auto-close per `docs/AGENT_PROMPT_orchestrator.md` Review nits).

---

## 1. Scope recap

This review validates alignment among:

- **`docs/MVP-checklist.md`** section **`## 0v4`**, including the new **`[x]`** row for the **demo CY policy-matrix operator harness** (`scripts/cy_cluster_policy_matrix_e2e.ps1`, **`pwm-testing`** + **`cq_process_ctl`**, PASS / exit 0 boundary) and its explicit statement that this is **not** a multi-user / regression / soak layer.
- **`docs/plans/mvp_v4.md`** — appended paragraph under code/doc closeout: **демонстратор** vs backlog for soak / multi-user expansion and broader policy cases growing with protocol.
- **`docs/runbooks/cy-cluster-policy-matrix-e2e.md`** — prerequisites, scripted scenario, **`cq_process_ctl`** delegation pattern, limits (PowerShell 5.1 / brute timing).
- **`docs/runbooks/demo-devnet-quickstart.md`** — demo/devnet disclaimers; optional V4 policy operator path (**§6.1**).
- **`CHANGELOG.md`** — **`2026-05-17T17:00Z`** CY cluster policy-matrix harness vs **`2026-05-17T12:45Z`** devnet policy E2E harness; narrative consistent with checklist deferrals.
- **`tasks/20260517-cy-cluster-policy-matrix-e2e-live.json`** — records failed iterations then **final PASS** with artifacts; matches CHANGELOG “Verified” block.
- **Spot-check:** **`README.md`** / **`README-ru.md`** — PoA devnet framing; no explicit claim of multi-user cluster soak or full workspace CI as a demo gate.

---

## 2. Requirements fit

**Stated owner intent** (working demonstrator; narrow matrix smoke; no implication of production / long-lived cluster simulation beyond specs and checklist) is **reflected consistently** in the checklist row, **`mvp_v4.md`** addendum, CY runbook “Ограничения”, and **`CHANGELOG`** qualifiers.

**Covered by documented gates:**

- **V4 integrated gate** (unit-focused scope): see **`docs/MVP-checklist.md`** **`[x]`** integrated V4 row — `cargo fmt --check`, `cargo check --workspace`, `cargo test -p pwmd --lib`, `cargo test -p pwm-core --lib`, full **`pwm-cli`**, policy filters and snapshot bench compile; smoke report cited there.
- **Thin live operator harness for policy matrix on CY two-node cluster:** checklist **`[x]`** row + **`CHANGELOG`** **`2026-05-17T17:00Z`** + CY runbook + closed ticket with PASS delegation.

**Explicitly deferred / partial (`[~]` and narrative):**

- **Full `cargo test --workspace`** — **`docs/MVP-checklist.md`** **`## 0v4`** last row **`[~]`**: optional hardening before a **public testnet announcement**, not positioned as required for this demo publication slice.
- **Manual TUI operation and long-running devnet soak** — same **`[~]`** row; not part of V4-6 integrated narrative for this demonstrator cut.
- **Multi-user activity simulation / extended policy-case matrix beyond the scripted smoke** — explicitly called out in the new checklist row and in **`mvp_v4.md`** demonstrator paragraph as **follow-on** as protocol and tickets grow.

**No contradiction found** between “**`POST /v1/tx`** carries V4 policy flow” in README and the narrower **CY matrix harness**: README describes capability and API contract; checklist/Changelog distinguish **integrated/unit-focused gate** vs **optional/long operator harnesses**.

---

## 3. Style and module shape

Documentation-only slice — **no production Rust** reviewed. Runbooks use clear warnings (demo keys, not production security posture; PowerShell stderr pitfalls). Checklist legend and traceability tables remain readable.

### Wire JSON / u128

Wire JSON / u128: **not applicable** (no peer wire / RFC wire contract changes in this documentation slice).

---

## 4. Safety (trust boundaries for external readers)

- **Demo/devnet posture** is stated in **`demo-devnet-quickstart.md`** (passphrases, local demo material).
- **Operator/debug surfaces** (e.g. log-control RPC) are called out in README as **outside stable public API** — reduces mis-reading by integrators.
- **Harness operational risks** (long brute, **`cq_process_ctl`** timeouts, cleanup) are documented in CY runbook — appropriate for operators, not hidden.

---

## 5. Tests (what the docs claim vs what is deferred)

| Claim in docs | Role |
|---------------|------|
| V4 integrated gate (fmt, workspace check, **`pwmd`/`pwm-core --lib`**, **`pwm-cli`**, snapshot bench compile) | **Core** reproducible bar for the policy runtime slice |
| **`devnet_v4_policy_e2e.ps1`** (quickstart §6.1; CHANGELOG 12:45Z) | Devnet-oriented live policy smoke |
| **`cy_cluster_policy_matrix_e2e.ps1`** (checklist row; CHANGELOG 17:00Z; CY runbook) | **Narrow** CY two-node operator matrix smoke |
| **`cargo test --workspace`**, manual TUI soak, long-running soak | **Explicitly not** part of this demo publication contract (`[~]` / plan deferrals) |

---

## 6. Verdict

**PASS_WITH_NITS**

### Nits (classification)

**Mechanical (orchestrator may auto-close without owner poll):**

1. **`docs/runbooks/demo-devnet-quickstart.md` §6.1** documents **`devnet_v4_policy_e2e.ps1`** and pwm-testing/**`cq_process_ctl`** well, but does **not** point readers to **`docs/runbooks/cy-cluster-policy-matrix-e2e.md`** / **`scripts/cy_cluster_policy_matrix_e2e.ps1`** for the **two-node CY cluster policy-matrix** path now traced in **`MVP-checklist.md`** and **`CHANGELOG`**. Add a short cross-link sentence so external readers landing only on the quickstart discover the matrix harness without assuming it replaces the integrated unit gate.

**Escalation (owner / product):** **none** identified in this slice — no request to change protocol promises, security posture, or acceptance contracts beyond clearer navigation.

---

## 7. Risks / open items for external readers (one screen)

1. **Scope:** treat the snapshot as **demo/devnet + bounded harnesses**, not load-tested multi-user production or guaranteed long soak stability.
2. **`cargo test --workspace`** remains **optional hardening** per checklist **`[~]`** — integrators should not infer full-workspace green from this publication slice alone.
3. **CY matrix script** can run **tens of minutes** (brute + cold cargo); operators need **`cq_process_ctl`** long timeouts and isolated **`CARGO_TARGET_DIR`** on Windows per testing canon.
4. **Ticket history** on **`20260517-cy-cluster-policy-matrix-e2e-live.json`** shows earlier FAILs before PASS — readers should rely on **final delegation + CHANGELOG** narrative, not first-line FAILURE stubs.
5. **README** summarizes broad MVP capabilities; for **demo reproducibility**, prefer **runbooks + checklist** as the contract for what was explicitly executed vs deferred.

---

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260517-demo-publication-readiness-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 9500
  confidence: low
```

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260517-demo-publication-readiness-review.md'
git add 'tasks/20260517-demo-publication-readiness-review.json'
git commit -m 'docs: demo publication readiness review (PASS_WITH_NITS mechanical)'
```

**One-line verdict for orchestrator:** **PASS_WITH_NITS** — mechanical doc cross-link nit only; **`tasks/20260517-demo-publication-readiness-review.json`** → **`done`**.
