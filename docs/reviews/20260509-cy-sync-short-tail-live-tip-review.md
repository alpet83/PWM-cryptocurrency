# Ревью CY-lab: «залипание на 100%», хвост после long sync и ощущение докачки «на каждый блок»

## 1. Scope recap

- **Оператор:** после длительной синхронизации процесс оставался на **100%**, новые блоки **не подтягивались** до перезапуска; после рестарта короткий sync прошёл быстро, но далее **на каждый новый блок** снова проявлялась активность синка — создаётся впечатление, что **live-обновление tip с соседа не успевает** и срабатывает **докачка короткого хвоста**.
- **Анализ:** рантайм-хвосты `terminals/1.txt` (proposer), `terminals/2.txt` (attester standby) и реализация `crates/pwmd/src/transport/peer_session/sync_live.rs` + точки вызова `SyncTipAnnounce` в `steady_session.rs` / `inbound.rs`.
- **Цель:** зафиксировать, есть ли **нарушение short-tail** или это **смесь штатного поведения, логирования и ранее описанной конкуренции** с кластером/очередью чтения.

## 2. Requirements fit (ожидание vs код и логи)

### 2.1. Короткий хвост в протоколе

- В **`on_tip`** при отставании от объявленного `head_h` параметр **`cup_req = lag >= SYNC_CUP_LAG_MIN || live_stall >= 2 || cup_on`**, где **`SYNC_CUP_LAG_MIN = 256`** (`sync_live.rs`). То есть при **лагe 1…255** и `live_stall < 2` используется **live-ветка**: **`ask_hdr`** для `local_h + 1`, затем цепочка **`on_hdr_batch` → ask_blk → `on_blk_batch`**, без полноформатного catch-up epoch.
- Вывод: «докачка на каждый блок» **не обязана** означать повторный CUP; для малого хвоста это **ожидаемый запрос одного заголовка/блока на шаг** (но см. §2.3 про интерпретацию логов).

### 2.2. Лог attester: `mem`, `disk`, `goal`, «100%»

- Строка **`Sync progress`** печатается из **`maybe_log_sync_prog` → `sync_prog_tick`** (`sync_live.rs`). В сообщении **`mem`** — фактический **`chain.tip_h()`**, **`goal`** — видимый **`peer_tip_h`** (в тике передаётся как верхняя граница прогресса), **`disk`** — **`last_snapshot_height`** (персист).
- В ваших хвостах (`terminals/2`, ~10:44–10:47) типично: **`mem=goal`**, **`rem=0`**, **`pct=100`**, при этом **`disk` отстаёт на несколько высот** до ближайшего **`standby sync checkpoint`** с шагом **`STANDBY_SYNC_FLUSH_BLK_IV`** (10). Это **соответствует коду** `apply_blk_batch`: для **`SealRole::Standby`** периодический flush срабатывает на высотах **`h % 10 == 0`**, а не на каждом блоке — **нарушением short-tail это не является**.

### 2.3. Почему «100%» и Sync progress мелькают **каждый блок** (важно)

Механизм **`sync_prog_tick`** (тот же файл):

- При **`rem > 0`** (tip ушёл вперёд, локально ещё не догнали), если **не** выполнены условия логирования (в т.ч. из‑за **`SYNC_PROG_MIN_MS = 7000`**), выполняется ветка:

```rust
if !(done_now || (time_ok && (pct_ok || lag_resume) && snap.rem > 0)) {
    if snap.rem > 0 {
        st.sync_log_done = false;
    }
    return None;
}
```

- То есть при **кратковременном отставании на один блок** и **раннем возврате без лога** флаг **`sync_log_done` принудительно сбрасывается в `false`**. Сразу после применения блока снова **`rem = 0`**, срабатывает **`done_now = (rem == 0 && !sync_log_done)`**, и снова печатается **`Sync progress 100%`** — **каждый раз при новом блоке**, даже при быстрой live-догонке.

**Итог по симптому оператора:** плотная череда **`Sync progress 100%`** **не диагностирует сама по себе** повторный CUP или отсутствие live-tip; это **согласовано с текущей логикой троттлинга/сброса `sync_log_done`**. Для вывода о режиме нужны строки уровня **`pwmd::peer`**: **`peer sync on_tip live_hdr`**, **`cup_started`**, **`peer sync catchup progress`**.

### 2.4. «Залипло на 100%, свежие блоки не учитывались»

По коду «**100%**» в прогрессе означает лишь **`local_h >= goal`** от последнего известного **`peer_tip_h`** в контексте логгера, а не гарантию «мы на глобальном лучшем tip сети».

Наиболее правдоподобные классы причин (для проверки по **peer-логам** и метрикам, не догадки как факт):

1. **Перестали доходить или обрабатываться `SyncTipAnnounce`** (застой в multiplexing, обрыв сессии, приоритет других фреймов) → **`st.tip_h` и `goal` не растут**, визуально «мы на 100%», но сеть уже впереди.
2. **`live_stall`** нарастал (несовпадение hdr/batch, пустые block в wire, конфликт fork) → при **`live_stall >= 2`** включается ветка **CUP** даже при малом лаге; при сбоях CUP возможен **застой**, пока не сработает backoff / reconnect.
3. **`cup_active`** остаётся истинным — тогда **`on_hdr_batch` / `on_blk_batch` ранний return** игнорирует live-хвост (см. проверки `cup_active` в этих функциях).
4. Ранее в проекте уже фиксировались **`wire_decode_failed: u128 is not supported`** и гонки handshake/snapshot — это **отдельный класс** «ложного конца синка»; при новом срезе смотреть наличие **`wire_decode_failed`** в логах.

Без приложенного **peer-лога окна «залипания»** нельзя зафиксировать единственный корень; по **текущему** хвосту терминалов видно **нормальное продолжение seal и рост высот** у proposer и **догон standby по mem** у attester.

## 3. Style and module shape

- Прод-код в этом слайсе **не менялся**; форма модулей **не оценивается** (кроме замечания: компактная ветвь `on_tip` / `sync_prog_*` уже несёт нетривиальную политику — её стоит сопровождать **операторским абзацем в RFC/руководстве**).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice); historically unsafely encoded `u128` on JSON peer wire remains a **known stall class** for sync — cite incident docs if diagnosing «stuck at 100%» in the field.

## 4. Safety

- **Операционный риск:** путаница «**100% = конец синка**» при **устаревшем `peer_tip_h`** может задержать реакцию оператора; **`disk << mem`** в standby **не баг**, но без документации выглядит как «несогласованность».
- **Self-DoS / coupling:** при **том же TCP** и насыщении catch-up (см. `20260513-cy-lab-sync-vs-cluster-priority-review.md`) **tip-оповещения** могут приходить с задержкой — усугубляя ощущение «нет live-режима».

## 5. Tests

- Есть модульные тесты **`sync_prog_tick_*`** на троттлинг и **`lag_resume`**; **нет** явного теста на «после `rem>0` без лога всегда следует `done_next` при быстром догоне» — поведение получается **побочным эффектом** троттлинга; для pwm-coding: рассмотреть тест на **отсутствие спама 100%** при типичном `block_time` или на **явный смысл флага `sync_log_done`**.
- Интеграционно полезен сценарий: **standby + один seed**, метрики **`sync_tip_seen_total`**, **`sync_apply_ok_total`**, **`goal - local_h`** во времени.

## 6. Verdict

**PASS_WITH_NITS** для «нарушения short-tail» в смысле **ошибочного алгоритма хвоста <256**: по коду **короткий хвост обрабатывается через live hdr/blk**.  
**REQUEST_CHANGES** на уровне **продукта/наблюдаемости** (для pwm-coding / RFC, не в этом коммите):

1. **Развести в логах «режим»**: одна строка на блок с полями **`live_hdr` vs `cup`** и/или счётчик «запросов hdr на height» без спама `Sync progress 100%`.
2. **Пересмотреть `sync_prog_tick`:** не сбрасывать **`sync_log_done`** в ветке throttle таким образом, чтобы **каждый блок** вынуждал «снова done»; либо **логировать 100%** только при **изменении goal** или раз в `SYNC_PROG_MIN_MS` при `rem=0`.
3. **Документировать standby:** **`mem` vs `disk`** и периодический flush — чтобы «100% при отстающем disk» не трактовалось как баг.
4. **На RFC / ops:** явное ожидание: при **lag и неполном кворуме** задержка **tip** на peer-сессии возможна; критерии reconnect / health.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260509-cy-sync-short-tail-live-tip-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 9500
  confidence: low
```

GLOSSARY.md: без изменений (нового жаргона для словаря вне этого отчёта не добавляли).

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260509-cy-sync-short-tail-live-tip-review.md'
git commit -m 'docs(review): CY lab short tail vs live tip and Sync progress logging'
```
