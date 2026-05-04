# Sprint 15 — ревью: P0 развод горячего JsonFile `save` от монолита

**Тикет:** `tasks/20260504-s15-snapshot-trust-default-api-save-split.json` (частичное закрытие acceptance).

## Scope recap

Изменение ограничено **архитектурным выводом** [`sprint-15-arch-trust-checkpoint-rescan-review.md`](./sprint-15-arch-trust-checkpoint-rescan-review.md) пункт **P0**: пути HTTP/`snapshot_save_under_inner_lock` не должны собирать монолитный JSON через полное чтение epoch-файлов при уже включённом режиме эпох на диске.

## Requirements fit

**Выполнено:** при наличии `epochs/pwm-epochs-manifest.json` вызываются `sync_epoch_disk_to_tip` + `save_checkpoint_summary` (`json_file_runtime_persist` в `io.rs`); ветка без manifest сохраняет прежний `save_snapshot` для legacy inline.

**Не в этом PR:** быстрый старт без полного replay (`load_snapshot`), флаги `--verify-chain`, отдельные константы интервалов CH vs JsonFile — остаются в тикете как следующие этапы.

## Safety

Поведение доверия к диску не менялось относительно предыдущего горячего save: по-прежнему записываются актуальный state/roaming/cross_shard и синхронизация хвоста в epoch JSONL перед summary.

## Tests

Добавлен `runtime_persist_after_disk_lag_loads` в `incremental.rs`. Прогоны: `cargo test -p pwmd`, `cargo test -p pwmd --features clickhouse-snapshot`.

## Verdict

**Approve** для объёма P0. Оставшиеся пункты тикета — отдельные PR (RFC + startup trust-default).

---

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-runtime-persist-P0-review.md
token_usage:
  source: estimate
  total: 2800
  confidence: low
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/sprint-15-runtime-persist-P0-review.md'
git add 'tasks/20260504-s15-snapshot-trust-default-api-save-split.json'
git commit -m 'docs(s15): P0 runtime persist review + ticket update'
```
