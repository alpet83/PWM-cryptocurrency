# Sprint 14 — Slice 12 (Docs) Testing Report

Дата: 2026-04-28  
Репозиторий: `P:/opt/docker/pwm-protocol`  
Тип: docs verification (no code tests)

## Scope

- Проверить согласованность guidance по портам в `docs/GENESIS_BLOCK.md`.
- Проверить, что обновлённые review docs отражают текущие контракты.
- Проверить, что ссылки tester-guide в `docs/MVP-checklist.md` указывают на существующие файлы.

## Checks and results

1. **GENESIS_BLOCK port guidance — PASS**
   - В разделах запуска есть два примера `--listen` (`127.0.0.1:3030` и `127.0.0.1:3040`).
   - В разделе проверок API используются `http://127.0.0.1:<listen_port>/...` и явное правило "используйте тот же порт, что в `--listen`".
   - Рассинхрона `3030/3040` в runtime-check шагах не обнаружено.

2. **Updated review docs vs current contracts — PASS**
   - `docs/reviews/genesis-validator-key-roles-20260428.md` отражает активный контракт `m/1000000'/1'` для validator key и оставляет `m/0'/0'` только как legacy/исторический контекст.
   - `docs/reviews/wallet-schema-version-behavior-20260428.md` явно помечен как исторический снимок и содержит актуальную ремарку, что create-paths теперь сохраняют wallet сразу в schema v3.
   - Эти формулировки согласованы с текущими docs-контрактами в `docs/GENESIS_BLOCK.md`, `docs/pwmd.md`, `docs/pwm-cli.md`.

3. **MVP-checklist tester-guide links — PASS**
   - В `docs/MVP-checklist.md` используются ссылки:
     - `./tester-guide-devnet-smoke.md`
     - `./tester-guide-cli-tui-scenarios.md`
     - `./tester-guide-env-errors-recovery.md`
   - Все три целевых файла существуют в `docs/`.

## Verdict

**APPROVE** — slice12 docs fixes валидны. Проверенные документы согласованы между собой и с текущими контрактами; битых tester-guide ссылок не найдено.
