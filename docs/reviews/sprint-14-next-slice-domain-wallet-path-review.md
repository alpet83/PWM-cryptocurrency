# Sprint 14 Next Slice Review (Pre-task)

## Scope
- `addr-derive` vs `addr-bruteforce` (включая deprecation-путь)
- default `--wallet-out` (`~/.pwm-crypto/default-wallet.yaml`) + Windows path translation + auto-mkdir
- cluster-aware resume вместо глобального `max(derivation_index)+1`
- ревизия `country_code_label` и переход к явному `default_domain_cluster_u8`

## Findings
- Разница между `addr-derive` и `addr-bruteforce` не только в stateless-режиме:
  - `addr-bruteforce`: label-only + `HighByteOnly` + flags policy + resume.
  - `addr-derive`: full-domain derive без resume, теперь может писать в wallet.
- `load_wallet_resume_start_index` сейчас действительно берёт глобальный максимум индекса без учёта целевого кластера.
- `country_code_label` не участвует в ключевых runtime-решениях и выглядит как legacy-метаданные.
- Default `--wallet-out` и auto-create каталога сейчас не централизованы как обязательный контракт.

## Recommendations
1. Soft-deprecate `addr-derive` в следующем слайсе (warning + docs/help), удаление позже отдельной чисткой.
2. Ввести единый resolver пути:
   - по умолчанию `~/.pwm-crypto/default-wallet.yaml`;
   - корректная обработка `~`/home на Windows и Unix;
   - `create_dir_all(parent)` перед записью.
3. Сделать cluster-aware resume:
   - сначала искать `max index` среди аккаунтов, совместимых с target cluster;
   - если совпадений нет — fallback к global max + 1.
4. Прекратить запись `country_code_label` в новые сохранения (оставить read-compat),
   и при необходимости ввести `default_domain_cluster_u8` с явным precedence:
   - explicit derivation params >
   - explicit `--country` >
   - wallet `default_domain_cluster_u8` >
   - понятная ошибка при невозможности выбора.

## Acceptance checklist
- [ ] Default wallet path resolver + `~` expansion + Windows compatibility
- [ ] Auto-create parent directory before wallet write
- [ ] `addr-derive` soft-deprecation notice (без ломки обратной совместимости)
- [ ] Cluster-aware resume + тесты mixed-domain wallet
- [ ] `country_code_label` больше не пишется в новые wallet, чтение старых сохраняется
- [ ] (Опционально) `default_domain_cluster_u8` + documented override rules

## Verdict
`approve with nits` для pre-task; риск следующего слайса — **medium** (изменения UX/CLI контрактов и resume-алгоритма).
