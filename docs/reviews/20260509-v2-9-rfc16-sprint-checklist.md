# Sprint V2-9 — RFC 16 Cluster Attestation Checklist (оркестратор + pwm-testing + pwm-review)

**Дата:** 2026-05-09 (обновление закрытия: 2026-05-22)  
**Статус:** спринтовой тикет `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json` закрыт (`done`); финальная фиксация в плане — `docs/plans/mvp_v2.md` § Sprint V2-9 «Статус закрытия спринта». Текст ниже — операторский baseline + долговая фиксация. Долгий смок на Windows: скрипты `cy-cluster-*.ps1` в корне репозитория (шарда CY).

## 1) Scope и референсы

- План спринта: `docs/plans/mvp_v2.md` (раздел **Sprint V2-9**, слайсы A/B/C, acceptance, наследование от V2-8 Slice 6).
- Нормативная база: `docs/rfc/16-validator-clone-attestation.md` (Variant A, §6.1, §8.1, §10, §11, §16).
- Дизайн-контекст S3: `docs/reviews/20260511-single-sealer-S3-cluster-consensus-design.md`.
- Локальный долгий смок (Windows): `cy-cluster-common.ps1` + `cy-cluster-proposer.ps1` / `cy-cluster-attester.ps1` / `cy-cluster-follower.ps1` (шарда CY, тот же genesis что у `node-1.ps1`).
- Модель запуска: feature-flag first, default off для публичного testnet до отдельного owner-gate.

## 2) Decision rows (зафиксировать до merge первых код-слайсов)

| Decision | Значение для V2-9 | Статус |
|---|---|---|
| Транспорт attest/propose (RFC §10) | **Расширение peer wire + capability bit(s)**; сообщения аттестации/раунда только поверх **уже установленных пиринговых сессий** (тот же транспорт, что и обычный same-shard peer flow). Отдельный REST/OOB-канал для «постройки кластера» **не** является целевым путём для V2-9. | `LOCKED` |
| Знакомство / членство кластера | Только внутри **пиринговых подключений** (handshake/capability уже согласованы для пира); нет автономного «обхода» пира для join кластера в этом слайсе (см. defer dynamic join, RFC Appendix B.5). | `LOCKED` |
| Имена feature/config/CLI | Ввести placeholders и не считать их frozen: `--cluster-role` (**TBD — confirm with owner**), `cluster_membership` (**TBD — confirm with owner**), `cluster_quorum_k` (**TBD — confirm with owner**), `cluster_tx_catchup_ms` (**TBD — confirm with owner**) | `OPEN` |
| Коммитер после кворума | Один designated committer (обычно proposer), остальные не seal-ят тот же `(H,R)` | `LOCKED` |
| Ортогональность S2 и quorum (RFC §8.1) | Lease/fencing отвечает за эксклюзивность seal; quorum membership/rotation отвечает за attest; не смешивать метрики и runbook-термины | `LOCKED` |
| Лимит консенсусного кластера на первую итерацию | Не более 3 узлов (RFC §7.2), только 2-of-2 и 2-of-3 дорожки | `LOCKED` |
| UDP / широковещание | **Отложено** (не Slice A–C поверх TCP peer wire). При введении datagram/broadcast-плоскости — **обязательны** подписанные кадры + анти-replay: нет опоры на защиту TCP-сессии (RFC §10.1 connectionless trust boundary). До отдельной спеки — не считать UDP несущим кворумный трафик. | `DEFERRED` |

## 3) Slice checklist (A / B / C)

### Slice A — ядро кластера (≤3 узла, flags, seal + S2, §6.1 logging)

| Пункт | Что должно быть зафиксировано в слайсе |
|---|---|
| Роли и конфиг | Роли `proposer/attester` вводятся через config/CLI placeholders (имена не финализировать без owner-confirm). |
| Quorum и размеры | Валидировать `n<=3`, `k` в допустимых границах, явная семантика `k-of-n` для 2 и 3 узлов. |
| Seal path и S2 | Seal без кворума запрещён при включённом профиле; при конфликте lease vs quorum — no-seal (RFC §8). |
| §6.1 bounded catchup | Обязательный bounded `T_tx_catchup <= T_attest`, с отказом `missing_tx_after_catchup` при таймауте. |
| §6.1 observability | Обязательный structured log для deferred-path (`attest_tx_lag` или эквивалентный ключ). |
| Транспортное решение | Реализация — **wire + capability**; трафик attest/propose только между настроенными пирами (секция 2). |

### Slice B — волны 2-узлового кластера

| Волна / сценарий | Минимальный gate |
|---|---|
| Happy path 2-of-2 | При наличии кворума блок seal-ится и воспроизводимо публикуется. |
| Negative: нет кворума | Нет seal, диагностический reason (`quorum_timeout`/эквивалент) виден в логах. |
| Fault inject по RFC §11 | Минимум один fault-сценарий (mute attest, invalid candidate, partition-lite) с ожидаемым no-seal/round_failed. |
| Артефакт воспроизведения | Автотест или runbook-шаги с фиксируемыми checkpoint-точками (height/hash). |

### Slice C — волны 3-узлового кластера + follower той же шарды вне кластера

| Волна / сценарий | Минимальный gate |
|---|---|
| Happy path 2-of-3 | Seal разрешается только после достижения требуемого кворума. |
| Degradation | Потеря одного attester-а не должна silently bypass-ить кворумную политику. |
| Topology: cluster + non-cluster follower | Нода той же шарды без cluster-ролей догоняет tip и удерживает согласованное состояние с источником блоков. |
| Convergence checks | Зафиксировать контрольные сравнения: `height`, `tip hash`, `state snapshot` на согласованных stop points. |
| Зависимость от V2-8 sync | Если same-shard sync baseline из V2-8 доступен — использовать его как основу; если нет, добавить явную **lab baseline row** и отдельный подчинённый блокер-таск. |

## 4) Out of scope (жёсткий defer для V2-9)

- Appendix B.5 dynamic cluster join.
- RFC §12.4: выбор `k` из большого relay pool (post-3-node этап).
- UDP / connectionless cluster broadcast без нормативной схемы **подписи + replay** (см. RFC §10.1) — не часть приёмки Slice B/C на TCP wire.

## 5) Acceptance / Demo gate (привязка к `mvp_v2` V2-9)

- За флагом проходят happy-path 2-of-2 и 2-of-3, негативы `нет кворума -> нет seal`.
- Прогоны двух- и трёхузловой волн документированы (автотест и/или runbook с воспроизведением).
- Для follower-ноды same-shard вне кластера подтверждена сходимость к ожидаемому shard tip/state.
- Логи содержат событие по §6.1 (deferred tx material path) при соответствующем сценарии.
- Документация обновлена: runbook note + ссылка на RFC 16, и явно отмечено default off.

## 6) Роль оркестратора: порядок исполнения

- Перед стартом кода закрыть оставшиеся **OPEN** строки секции 2 (в первую очередь **имена** feature/config/CLI и freeze-policy placeholder’ов). Транспорт и пиринговая модель кластера — **LOCKED** (wire + capability, только внутри peer-сессий). **Slice B/C** в этом спринте опираются на тот же TCP peer path; строка **UDP / широковещание** в §2 — **DEFERRED** до отдельной спеки (подписанные datagram’ы, см. RFC 16 §10.1).
- Делегация по слайсам: `pwm-coding -> pwm-testing -> pwm-review` без пропуска звеньев.
- В `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json` вести статус по слайсам и ссылкам на review-артефакты.

## 7) Next agents (handoff после старта реализации)

- `pwm-testing`: проверить wave-gates (2-node и 3-node), негативы no-quorum/fault-inject и convergence follower-ноды по height/hash/state.
- `pwm-review`: проверить соответствие RFC 16 (§6.1, §8.1, §10, §11) и отсутствие подмены S2-lease semantics кворумной логикой.

## 8) Наследование от V2-8 Slice 6 (краткая фиксация)

Цели wave-pack из V2-8 Slice 6 считаются перенесёнными и **перекрываются** этим спринтом на новой модели `single proposer + attest` (см. `docs/plans/mvp_v2.md`, Sprint V2-8/V2-9 и блокер `tasks/20260508-v2-sprint8-slice6-automated-waves.json`): legacy multi-sealer gate не является целевым acceptance-контрактом для V2-9.
