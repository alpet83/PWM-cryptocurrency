# Отчёт: снятие legacy shard-a/shard-b и место `DevLane` относительно RFC

**Дата:** 2026-05-10  
**Контекст:** владелец проверяет, не получилась ли «перелицовка» старой ошибки (две ноды A/B) вместо соответствия RFC и модели множества шардов.

## 1. История чата и тесты S2

- Полная «лента» диалога за день ассистентом не восстанавливается; ориентир — UI Cursor / экспорт transcripts.
- По тикету **`tasks/20260509-single-sealer-failover-profiles.json`** срез **S2** в делегациях зафиксирован как **таргетированный** прогон, **не** полный `cargo test -p pwmd`:
  - `lease_renew_ok_same_owner`, `lease_takeover_after_timeout`, `old_active_blocked_without_lease`;
  - плюс smoke: `status_exposes_identity_signals`, `prod_seed_idle_windows_ok`, `peer_micro_idle_hb_ok`.
- **Полный** прогон **`cargo test -p pwmd`** (303 passed) относится к отдельному тикету выравнивания HTTP-тестов под **`cluster_domain_hi`** после удаления compat-alias (**`tasks/20260509-pwmd-domain-hi-test-align.json`**, коммиты вроде `0770b4e`).

Итого: «все тесты слайса S2» в смысле **всех** юнитов/интеграций pwmd — **нет**, по тикету S2 проходили **перечисленные** фильтры + проверки из pwm-testing для S2.

## 2. Что именно убрали (legacy shard-a/b)

Удалены из поддерживаемого контракта:

- строковые namespace **`shard-a` / `shard-b`** и совместимые идентификаторы **`compat-shard-*` / `compat-node-*`**;
- режим **`RuntimeIdentityMode::Alias`** и захардкоженное отображение на эти строки;
- deprecated CLI **`--shard A|B`**;
- хранилище для alias-пути больше не использует отдельные «shard-*» каталоги — для explicit identity используется **`domain-hi-0xNN`**, для neutral — **`neutral/<listen-tag>`** (как в RFC-8 narrative).

Кодовая точка входа по identity: `crates/pwmd/src/identity.rs` (`Explicit` | `Neutral`, `storage_namespace` от `cluster_domain_hi` или neutral).

## 3. Это всё ещё «две ноды», только переименованные?

**Не в смысле протокольного числа шардов.**

- **`DevLane` (`Lane0` / `Lane1`)** — это **внутренняя метка процесса pwmd** для двух **параллельных dev-сценариев** в тестах/bootstrap (две преднастроенные связки genesis/config lane ↔ локальное поле в `App`), а не заявление «в сети только два шарда».
- Нормативная привязка рантайма к «гео-шарду» в терминах RFC — **`cluster_domain_hi` (`u8`) + `cluster_id` + `node_id` + `network_id`** (см. **RFC 8 §2.1 / §4**): то есть идентичность кластера задаётся **явным байтом и строками**, без эвристик split по диапазонам.

Множество независимых кластеров/«runtime shard instances» в модели RFC — это **много различных значений `domain_hi` / полной доменной семантики**, а не enum из двух вариантов в продакшен-конфиге.

## 4. Где «бинарность» всё ещё видна в коде

- **`tx_policy::shard_for_phase1_account`** по-прежнему относит домен аккаунта к **`DevLane::Lane0` или `Lane1`** по **классу домена Phase1** (Regulatory vs Sector в `pwm_core::domain_index`). Это **process-level карта для локальных guards**, а не утверждение «в мире PWM два шарда». Комментарии в `tx_policy.rs` прямо разделяют это от протокольного same-shard routing.
- Если цель — убрать даже эту двузначную абстракцию из названий/API в пользу только «domain class → guard», это уже **отдельный рефакторинг** (нейминг + возможное разложение на типы классов), не блокирующий факт удаления строк `shard-a/b`.

## 5. Соответствие RFC (краткий вывод)

- **Да:** запуск и namespace для supported operator path идут через **explicit domain-first identity** и **`domain-hi-0xNN`**, без fake `shard-a/b` — это **в сторону** RFC-8.
- **Не автоматически «брак»:** сохранение **`DevLane` как двух dev-лейнов** не отменяет модель «сотни доменных кодов / ~200 стран и т.д.» в ядре — эти коды живут в **`pwm_core::domain_index`**, а не в enum из двух значений для прод-идентичности.

## 6. Рекомендация по «общему ревью»

Если нужна формальная заявка для merge/релиза: заказать **`pwm-review`** узко на формулировку «DevLane vs RFC-8 / отсутствие регресса к бинарной модели шардов», с входными артефактами: этот файл + `docs/rfc/8-shard-runtime-identity-and-peering.md` + `crates/pwmd/src/identity.rs`.
