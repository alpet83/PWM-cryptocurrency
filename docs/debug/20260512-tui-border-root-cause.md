# TUI active panel border: WT PowerShell debug report

Ticket: `tasks/20260512-tui-wt-border-debug.json`  
Agent: `pwm-debug`  
Verbosity focus: `tui:border`

## Repro

- Operator repro: Windows Terminal -> PowerShell profile -> launch `pwm-tui`; black background is correct, static yellow UI text is visible, but Owner/Receivers active border does not produce a visible yellow focus cue.
- Control context from review: `cmd.exe -> powershell` had previously shown the expected focus cue; after black-background/guard fix, the remaining failure is limited to the active panel border in the WT PowerShell launch path.
- I did not screen-scrape the TUI. I used source-level and buffer-level evidence only.

## Root Cause

The current product code does apply `border_style(fg(Color::Yellow))` to the active `Block`, and the current two-pass render (`Block` first, `Table` inside `block.inner(area)`) does not overwrite the border cells. The remaining regression is a terminal-palette/visual-weight issue: in ratatui 0.26.3, `ratatui::style::Color::Yellow` is converted by the crossterm backend to `crossterm::style::Color::DarkYellow`, which crossterm formats as ANSI indexed color slot 3 (`38;5;3`) rather than bright yellow slot 11. Windows Terminal profiles can map slot 3 to a brown/olive low-luminance color; on one-cell-wide Unicode box-drawing glyphs this can look unchanged or effectively invisible, while ordinary yellow text remains perceptible because text has more filled pixels and often appears on different local backgrounds.

## Evidence

- Product render path: `crates/pwm-tui/src/tui_loop.rs:1005-1024` and `crates/pwm-tui/src/tui_loop.rs:1074-1093` build Owner/Receivers blocks with active `border_style(fg(Color::Yellow))`, compute `inner`, render `Block`, then render `Table` into `inner`.
- Ratatui `Block` pipeline: `ratatui-0.26.3/src/widgets/block.rs:592-599` renders block style, then borders, then titles; `block.rs:628-699` writes every border cell with `self.border_style`.
- Ratatui `Table` pipeline: `ratatui-0.26.3/src/widgets/table/table.rs:599-623` renders the table only in the supplied area; in PWM this area is `block.inner(area)`, so it does not touch outer border coordinates.
- Buffer semantics: `ratatui-0.26.3/src/buffer/cell.rs:71-85` patches foreground/background/modifiers into cells; `buffer.rs:350-371` includes style changes in diff output.
- Backend color mapping: `ratatui-0.26.3/src/backend/crossterm.rs:250-272` maps `Color::Yellow` to `crossterm::style::Color::DarkYellow`; crossterm documents `Yellow` as light and `DarkYellow` as dark in `crossterm-0.27.0/src/style/types/color.rs:15-26`, and formats `DarkYellow` as `5;3` in `crossterm-0.27.0/src/style/types/colored.rs:131-140`.
- Temporary debug test: `active_panel_border_survives_table_inner_render` passed, proving `Block` border cells keep `fg(Color::Yellow)` and `bg(Color::Black)` after the inner `Table` render.
- Local environment note: the shell used for tool runs has `NO_COLOR=1`, which made one ANSI-string assertion return an empty string. This is a secondary environment risk, but it does not match the operator's reported WT scenario because static yellow is visible there.

## Commands Run

- `cargo test -p pwm-tui --lib` -> pass, 14/14.
- `git blame -L 1005,1083 -- crates/pwm-tui/src/tui_loop.rs; git log --oneline -S border_style -- crates/pwm-tui/src/tui_loop.rs` -> confirms original `border_style` lines are old (`de5c5826`), while current two-pass `inner` render is uncommitted working-tree code from the prior coding slice.
- Temporary `cargo test -p pwm-tui --test tui_border_debug -- --nocapture` -> first run exposed local `NO_COLOR=1`; adjusted env-independent assertion; second run passed 2/2.

## Instrumentation

- Added temporary file: `crates/pwm-tui/tests/tui_border_debug.rs` (one add-file hunk).
- Reverted: yes, file deleted before return.
- No production code changes were made by `pwm-debug`.

## Recommendation for pwm-coding

- Keep the two-pass `Block` then `Table(inner)` structure; it is the right composition hardening and was not the remaining fault.
- Change the active focus cue away from palette slot 3. Preferred minimal code hardening: use `Color::LightYellow` or `Color::Indexed(11)` plus `Modifier::BOLD` for active borders, and apply the same active style to the title via `title_style` so the whole top focus cue is bright.
- If WT still makes thin glyphs hard to read, use a thicker/rounded active border type or add an explicit textual focus marker in the title, for example `"> Owner <"`, styled bright yellow/bold. This avoids relying solely on one-pixel box-drawing strokes.

## Recommendation for pwm-testing

- Add a non-screen-scraping regression test that renders `Block` plus `Table(inner)` into a ratatui `Buffer` and asserts active border cells have the expected foreground/background.
- Keep manual acceptance for WT/conhost visual contrast because actual palette/font antialiasing cannot be proven from stdout capture.

## Open Risks

- If the operator's Start-menu PowerShell profile sets `NO_COLOR`, crossterm may suppress ANSI colors entirely; the reported visible static yellow argues against this, but `Get-ChildItem Env:NO_COLOR` should be captured in the operator matrix.
- The final perceived contrast depends on the WT color scheme and font rendering; RGB or bright indexed color is more robust than semantic `Color::Yellow`.

## Participation

- `agent`: `pwm-debug`
- `result`: `PASS`
- `verbosity_focus`: `tui:border`
- `instrumentation`: `crates/pwm-tui/tests/tui_border_debug.rs`, 1 add-file hunk, `reverted: yes`
- `repro`: manual WT PowerShell operator repro plus deterministic buffer repro; `deterministic: yes` for buffer evidence, manual visual repro operator-verified
- `artifacts`: `docs/debug/20260512-tui-border-root-cause.md`
- `commands`: CQDS `cq_help`, CQDS `cq_files_ctl start_grep`, `cargo test -p pwm-tui --lib`, temporary `cargo test -p pwm-tui --test tui_border_debug -- --nocapture`, `git blame`, `git log -S`, env check
- `cleanup`: cleaned yes; temporary test deleted; no processes started
- `token_usage`: `{ "source": "estimate", "input": 22000, "output": 4500, "total": 26500, "confidence": "medium" }`
