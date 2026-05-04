# slice-artifacts.ps1

Lightweight helper for repetitive Sprint 6 slice artifact scaffolding.

## What it does

- `init`:
  - appends draft section blocks for `Slice #N` into:
    - `docs/reviews/sprint-6-checklist.md`
    - `docs/reviews/sprint-6-status-note.md`
    - `docs/reviews/sprint-6-review-report.md`
    - `docs/reviews/sprint-6-test-report.md`
  - adds missing JSON keys into `tasks/20260424-sprint6-optimization.json`:
    - `review_evidence_manifest_sliceN`
    - `mini_report_sliceN_coding`
    - `mini_report_sliceN_testing`
    - `mini_report_sliceN_review`
  - is idempotent: existing section headers/keys are not duplicated.
- `fill-diff`:
  - reads `git diff --numstat HEAD`
  - updates:
    - `review_evidence_manifest_sliceN.scoped_diff_stat`
    - `review_evidence_manifest_sliceN.generated_at`
- `patch-manifest-numstat`:
  - reads `git diff --numstat HEAD`
  - rewrites **only** the `scoped_diff_stat` JSON array + `generated_at` inside `review_evidence_manifest_sliceN` using UTF-8 raw text edits (no `ConvertTo-Json` for the whole task file)
  - intended for task JSON files that contain **Cyrillic** (or other sensitive formatting) where `fill-diff` is risky

## Sprint 6 evidence policy (`scoped_diff_stat`)

- **Default (recommended):** `fill-diff` / `patch-manifest-numstat` record only paths under **`crates/**`** and **`tools/**`**. Sprint markdown (`docs/reviews/sprint-6-*.md`) and the sprint task JSON are **excluded** from `scoped_diff_stat`: they are process bookkeeping and create self-referential metadata that goes stale as soon as the manifest is updated.
- **Opt-in full tree:** pass **`-IncludeArtifactPaths`** when you explicitly need every `git diff --numstat` path inside the manifest (rare).
- **Markdown review sections:** describe semantics, touched symbols, and risks — do **not** mirror per-file numstat for sprint artifacts; keep the narrative focused on code behavior.

## Sprint 6 orchestration (batching)

- If many tiny DRY wins remain, **batch ~3–4 coding changes per slice** before running the full closeout (single `cargo test -p pwmd`, one artifact sync, one `slice-commit` / manifest slice). This keeps review overhead proportional to delivered value.
- **`crates/pwmd/src/lib.rs` modular decomposition** (splitting into multiple `mod` files, moving large sections) is a **separate sprint / initiative** — do not fold it into the micro-slice conveyor; plan it explicitly when micro-DRY yields diminishing returns.

## Safety rules

- Uses marker validation before write; if a target marker is missing, it stops with a clear error and does not modify files.
- `-DryRun` prints planned updates only.

## Warning (task JSON + Cyrillic)

- `fill-diff` re-serializes the entire task JSON via `ConvertTo-Json`, which can reformat the file broadly.
- Prefer **`patch-manifest-numstat`** when you only need manifest `scoped_diff_stat` / `generated_at` and the task file contains non-ASCII notes.
- The script writes UTF-8 **without BOM** to reduce editor/tooling encoding surprises on Windows.
- If you see any corruption in Cyrillic strings, stop and restore the task file from git, then apply manifest updates manually (or use a JSON-aware patch workflow).

## Usage

Initialize slice artifacts (required: `-TouchedSymbols`):

```powershell
pwsh ./tools/slice-artifacts.ps1 `
  -SliceNumber 12 `
  -Mode init `
  -TouchedSymbols "helper_a(...)", "some_callsite(...)"
```

Initialize with custom no-change assertions:

```powershell
pwsh ./tools/slice-artifacts.ps1 `
  -SliceNumber 12 `
  -Mode init `
  -TouchedSymbols "helper_a(...)" `
  -NoChangeAssertions "tx-path/tx guards", "HTTP routes", "response fields"
```

Populate scoped diff stat in task manifest (code paths only by default):

```powershell
pwsh ./tools/slice-artifacts.ps1 -SliceNumber 12 -Mode fill-diff
```

Include sprint markdown + task JSON lines in the manifest (not recommended for routine slices):

```powershell
pwsh ./tools/slice-artifacts.ps1 -SliceNumber 12 -Mode fill-diff -IncludeArtifactPaths
```

Patch **only** manifest numstat fields (safer for Cyrillic-heavy task JSON):

```powershell
pwsh ./tools/slice-artifacts.ps1 -SliceNumber 15 -Mode patch-manifest-numstat
```

Dry run examples:

```powershell
pwsh ./tools/slice-artifacts.ps1 -SliceNumber 12 -Mode init -TouchedSymbols "helper_a(...)" -DryRun
pwsh ./tools/slice-artifacts.ps1 -SliceNumber 12 -Mode fill-diff -DryRun
pwsh ./tools/slice-artifacts.ps1 -SliceNumber 15 -Mode patch-manifest-numstat -DryRun
```

Custom task artifact path:

```powershell
pwsh ./tools/slice-artifacts.ps1 `
  -SliceNumber 12 `
  -Mode init `
  -TouchedSymbols "helper_a(...)" `
  -TaskArtifactPath "tasks/20260424-sprint6-optimization.json"
```
