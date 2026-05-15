# S1 single-sealer guards: final review (pwm-review)

Дата: 2026-05-09  
Тикет: `tasks/20260509-single-sealer-failover-profiles.json`  
Оцениваемые коммиты: `c9ecad6` (реализация), `1f30db6` (трассируемость тикета), `b9f2ee2` (pwm-testing отчёт)

## 1. Scope recap

Срез **S1** по артефактам и коду закрывает: профиль деплоя по умолчанию `single_sealer`, опциональный `multi_sealer_experimental`, сигналы идентичности в `NodeHello` и `/v1/status`, строгий отказ при same-validator **active/active** в режиме `single_sealer`, документацию (`docs/runbook-same-shard-sync-v1.md`, отчёт кодирования). Связь с **docs/MVP-checklist.md** §4 (seal loop / same-shard guardrails) и §12 (границы отложенного консенсуса): реализована **сигнальная и policy-часть** на handshake; полноценный lease/failover (S2) в этих коммитах не заявлен.

## 2. Requirements fit

**Профиль по умолчанию и CLI:** `PwmdConfig::default()` и clap (`--deployment-profile`, env `PWM_DEPLOYMENT_PROFILE`) согласованы на `single_sealer`. Название `multi_sealer_experimental` сопровождается warning на старте — соответствует замыслу «явный non-default».

**Гварды:** Логика в `process_incoming_peer_hello` срабатывает только для native same-shard, при совпадении `validator_identity_hash`, разных `node_instance_id`, профиле `SingleSealer` и обеих ролях `Active` — отказ с стабильным лейблом `same_validator_active_conflict` и инкрементом метрик. Для `active`/`standby` — приём; для `multi_sealer_experimental` при двух active — лог «allowed_experimental» без reject — соответствует ослаблению только вне strict single-sealer.

**Роль seal:** `derive_seal_role`: override из конфига → иначе при `debug_disable_seal_loop` → `Standby`, иначе `Active` — согласовано с runbook (fallback на standby через отключение seal-loop).

**Пробелы по охвату:** Same-validator policy **не применяется**, если у удалённого peer нет валидного `validator_identity_hash` в hello (ветка сравнения не входит) — для смешанных версий без полей идентичности остаётся операционный зазор до вывода старых клиентов или обязательных полей на wire.

## 3. Style and module shape

- Имена: `python scripts/check_rust_fn_name_segments.py` по затронутым путям pwmd (handshake, incoming_hello, lifecycle, main, bootstrap, handlers_status, dial, handshake_state) — **violations пусто**.
- `PWM_PROTOCOL_VERSION` остаётся `0.1.0`; добавлены поля в `NodeHelloCapabilities`, подпись покрывает сериализованный `capabilities` целиком. Требуется явная продуктовая позиция по wire-compat (см. Safety): решение «minor-only / без bump» не оформлено в отчёте кодирования как отдельный RFC-пункт, но для testnet/MVP часто приемлемо при отсутствии обязательства mixed-version.

## 4. Safety

- **Граница доверия:** Решение о same-validator опирается на подписанный hello и объявленные поля; подделка роли/хеша без компрометации ключа не рассматривается — в модели p2p это ожидаемо.
- **DoS:** Отказ по причине конфликта — ожидаемое поведение; деталь в логе содержит instance id и хеш для расследования.
- **Подпись и совместимость:** Payload подписи включает полный JSON `capabilities`. Пиры **до S1** с другим каноническим JSON (без новых ключей) при смешении версий могут давать рассогласование verify, если такие бинарники ещё существуют в поле. Риск **остаточный** для этапа, где допускается единая версия нод.
- **Паники:** В просмотренных путях guard использует saturating add для счётчиков; явных новых `unwrap` в горячем пути при ревью не выделено.

## 5. Tests

Целевые тесты из тикета покрывают: default профиль, active/active reject, active/standby allow, прокидывание полей в исходящий hello и в status. **pwm-testing** (`b9f2ee2`) фиксирует повторный прогон тех же проверок — согласовано.

Не покрыто автотестами: смешанная версия без identity-полей; поведение `multi_sealer_experimental` при двух active (только логирование) — низкий приоритет при явном experimental профиле.

## 6. Verdict

**approve with nits** — блокирующих дефектов в заявленном scope S1 не выявлено; остаточные риски задокументированы выше (wire mixed-version, пиры без validator hash).

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260509-s1-single-sealer-guards-final-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 4500, "confidence": "low" }
```

## 8. Merge readiness

- **Итог PASS / PARTIAL / FAIL:** **PASS** (с нитами, не блокирующими merge в рамках заявленного S1 и единой версии нод).
- **Готовность к merge:** **да** — при принятии остаточных рисков смешанных версий как вне scope или последующего hardening в S1.x/S2.

---

**Вердикт одной строкой для оркестратора:** `PASS — merge готов при единой версии wire; ниты: mixed-version и пиры без identity-полей.`
