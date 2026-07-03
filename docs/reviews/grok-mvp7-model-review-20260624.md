# Grok smoke: MVP V7 model routing review (pwm-review)

Дата: 2026-06-24  
Тикет: `grok-mvp7-model-review`  
Артефакт плана: `docs/plans/mvp_v7.md`, детализация S1: `docs/plans/mvp_v7s1.md`

## 1. Scope recap

Задача — **не код-ревью**, а рекомендация маршрутизации LLM по спринтам MVP v7. План фиксирует:

- **V7-S1** — perf-трек: SEDA pipeline, decoupling pre-seal от `Chain::seal`, gate ≥50 tx/s (`mvp_v7s1.md`, ADR 0013).
- **V7-S2** (план V7-1) — TUI read-only pending conservation.
- **V7-S3** (план V7-2) — `pwm-cli` addr-bruteforce v2 (occupied-skip + rayon MT + детерминизм).
- **V7-S4** (план V7-3) — emergency stake evacuation (ADR 0012) в `apply_tx`.
- **V7-S5** (план V7-4) — production offchain batch API (Merkle + on-chain anchor).
- **V7-S6** (планы V7-5 + V7-6) — devnet launch + BFT ADR-gate (код BFT вне V7).

Конвейер ролей (оркестратор): `pwm-info` → `pwm-coding` → `pwm-review` → `pwm-testing`; опционально `pwm-debug`, `pwm-polish` (frontier), closeout `pwm-optimus`.

**Нумерация:** в `mvp_v7.md` параллельно живут **V7-S1** (perf) и **V7-1…V7-6** (фичи). Ниже **V7-S1…V7-S6** — шесть рабочих спринтов: S1 = perf; S2…S6 = V7-1…V7-5 плюс V7-6 (ADR) в финальном S6.

## 2. Requirements fit

План явно требует:

| Трек | Глубина reasoning | Объём Rust | Review/QA акцент | Latency чувствительность |
|------|-------------------|------------|------------------|--------------------------|
| S1 perf | Очень высокая (concurrency, determinism) | pwm-core + pwmd, worktree | Сильный pwm-review + property/soak | Средняя (много итераций) |
| S2 TUI | Низкая–средняя | pwm-tui + узкий RPC | Стандартный contract gate | Высокая (мелкие слайсы) |
| S3 bruteforce | Средняя (алгоритм + MT) | pwm-cli только | Детерминизм «min index wins» | Высокая |
| S4 stake evac | Высокая (safety) | pwm-core `apply_tx` | Safety + validator set | Средняя |
| S5 offchain | Средняя–высокая (trust/crypto) | pwmd API + anchor | Merkle/wire boundaries | Средняя |
| S6 devnet+ADR | ADR: frontier; ops: средняя | scripts/harness + docs | Throughput evidence + ADR gate | Soak: низкая |

План **не** задаёт model tier в handoff — это пробел; рекомендации ниже закрывают его.

### Wire JSON / u128

Wire JSON / u128: not applicable (planning / model-routing review only; no peer wire slice).

## 3. Model inventory (PWM conveyor)

| Роль | Cursor subagent model | Companion / alt |
|------|----------------------|-----------------|
| `pwm-coding` | `gpt-5.3-codex` | VS Code bridge worker (same prompt) |
| `pwm-review` | `composer-2-fast` | **Grok** `acp_stdio` (`pwm_review` lane) |
| `pwm-testing` | `composer-2` | bridge `pwm-testing` |
| `pwm-debug` | `gpt-5.5-medium` | — |
| `pwm-info` | `kimi-k2.5` | mechanical discovery |
| `pwm-polish` / ADR drafting | frontier (host-defined) | delegate mechanical reads to haiku-class |
| `pwm-optimus` | `composer-2` | post-sprint only |

**Latency (Grok lane):** cold `grok -p` в git-репо ~35–45 s/вызов; **warm `acp_stdio`** обязателен для review-компаньона и частых pwm-review тикетов. Codex/Cursor Task не страдает от tarball-upload так же, как headless Grok.

**Cost heuristic:** frontier — только планирование, ADR, эскалация perf; codex — все продуктовые Rust-слайсы; composer-2-fast / grok-low — review; kimi — один info-тикет на спринт; composer-2 — testing.

## 4. Sprint → recommended model(s) → rationale

| Sprint | Primary implementation | Planning / docs | Review / QA | Rationale |
|--------|------------------------|-----------------|-------------|-----------|
| **V7-S1** Perf (SEDA) | **`gpt-5.3-codex`** | **Frontier** (slice plans, Mermaid, ADR 0013 alignment); `pwm-info` **`kimi-k2.5`** once per wave | **`Grok` `acp_stdio`** (pwm_review) **+** `composer-2-fast` on critical slices; **`composer-2`** testing; **`gpt-5.5-medium`** debug for soak/deadlock | Highest Rust complexity: channels, worker pool, determinism gate. Plan mandates strong review. Frontier justified for architecture before Slice 1; codex for implementation. Grok companion lane fits sustained concurrency review; avoid cold `grok -p` per slice. |
| **V7-S2** TUI conservation | **`gpt-5.3-codex`** | Orchestrator or **composer-2** (api-v1.md, runbook) | **`composer-2-fast`**; Grok optional | Narrow additive RPC + TUI poll UX; low reasoning depth; fast review tier saves cost; codex sufficient for Rust/UI glue. |
| **V7-S3** Bruteforce v2 | **`gpt-5.3-codex`** | **`composer-2`** (pwm-cli.md, runbook) | **`composer-2-fast`** (determinism focus) | Single-crate algorithmic work; MT correctness testable; no frontier needed unless occupied-set design disputed. |
| **V7-S4** Stake evac | **`gpt-5.3-codex`** | ADR 0012 already accepted — **`composer-2`** runbook only | **`Grok` `acp_stdio`** or **`composer-2-fast`**; **`composer-2`** testing | Safety-critical `apply_tx` path; prefer review tier with strong safety checklist; CY e2e via pwm-testing. |
| **V7-S5** Offchain batch | **`gpt-5.3-codex`** | **Frontier** for Merkle/trust model + client verify narrative; then codex for API shape | **`Grok` `acp_stdio`** (crypto/trust boundaries) + **`composer-2-fast`** | Mixed L7 API + on-chain anchor; frontier for ADR-level trust doc; codex for implementation; review must catch wire/API footguns. |
| **V7-S6** Devnet + BFT ADR | **`gpt-5.3-codex`** (genesis scripts, harness, docs tooling) | **Frontier only** for **V7-6 BFT ADR** (CometBFT vs custom vs Option A); **`composer-2`** for quickstart/runbooks | **`Grok` `acp_stdio`** for throughput gate evidence; **frontier co-review** on ADR before Accepted | V7-5 is ops/docs + soak; V7-6 is decision-only — frontier reasoning mandatory, **no codex for ADR prose**. Devnet gate depends on S1 results — debug (`gpt-5.5-medium`) for long ramp soaks. |

### Per-role defaults (все спринты)

| Activity | Recommended model | Notes |
|----------|-------------------|-------|
| Rust `crates/**` | `gpt-5.3-codex` | worktree_bridge для prolonged slices |
| `docs/plans`, sprint Mermaid | Frontier → orchestrator merges | Таймбокс; не codex |
| `docs/reviews`, ticket gate | `composer-2-fast` or Grok `acp_stdio` | S1/S4/S5/S6: prefer Grok companion |
| `cargo test`, checklist | `composer-2` | §Windows: isolated `CARGO_TARGET_DIR` |
| Discovery map | `kimi-k2.5` | Один `*-info.json` на волну слайсов |
| Flaky soak / ramp | `gpt-5.5-medium` | 15 min cap per debug prompt |
| Post-sprint bloat audit | `composer-2` (`pwm-optimus`) | После closeout только |

## 5. Safety (process)

- **Determinism gate (S1, S3):** не понижать review tier на слайсах с параллелизмом — риск пропуска ordering bugs.
- **ADR V7-6:** codex не должен писать Accepted ADR без frontier draft + owner sign-off.
- **Grok session/load:** тяжёлый `session_id` блокирует companion bootstrap — для pwm_review использовать `acp_session_mode=new` или лёгкий session; иначе review lane latency >> codex.

## 6. Tests

Не применимо к плану как коду. Для **smoke этого тикета**: таблица покрывает все шесть спринтов, роли и оси cost/latency/reasoning из brief.

## 7. Verdict

**approve with nits** — маршрутизация согласована с `mvp_v7.md` и текущим conveyor (`.cursor/agents/*.md`, `.cqds/grok_companion.toml`).

**Nits:**

1. Унифицировать нумерацию в handoff: явно `V7-Sn` ↔ `V7-n` mapping (таблица в §1).
2. Добавить в тикеты поле `model_tier` / `recommended_agent_models` при делегировании (сейчас отсутствует).
3. S6 разбить на два umbrella-тикета (devnet ops vs BFT ADR), если frontier ADR идёт параллельно codex harness — иначе конкурируют за внимание оркестратора.

## 8. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/grok-mvp7-model-review-20260624.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 8000, "confidence": "low" }
```

**Вердикт одной строкой для оркестратора:** `PASS — model routing table for V7-S1…S6; codex=Rust, frontier=ADR/planning, grok-acp=heavy review, composer-fast=light review, composer-2=test.`