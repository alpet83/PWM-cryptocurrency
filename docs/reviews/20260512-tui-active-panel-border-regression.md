# pwm-review: TUI — жёлтая рамка активной панели (Owner / Receivers)

## Запрос

Выявить **коммит**, после которого перестала визуально выделяться активная панель (жёлтая `border_style`), и при необходимости откатить.

## Метод

- Просмотр актуального кода: `crates/pwm-tui/src/tui_loop.rs`, функции `render_owner_panel` / `render_recv_panel`.
- `git blame` на строки `border_style` + `Color::Yellow`.
- `git log -S border_style -- crates/pwm-tui/src/tui_loop.rs`.
- Сопоставление с истории пина **ratatui** (`104dce3`: `0.28` → `0.26.3`).

## Выводы

1. **Отдельного «виновного» коммита, который убрал жёлтую рамку, в истории `tui_loop.rs` нет.** Логика `border_style(if active == Panel::...) { fg(Yellow) }` присутствует с выноса цикла в `tui_loop.rs` (**`e29e2d9`**, 2026-05-01) и вынесения рендера панелей (**`de5c582`**, 2026-05-02). `git blame` на строки рамки указывает на **`de5c582`**; после него нет правок, удаляющих эту ветку.
2. **Откат к «старому коммиту» не восстанавливает утраченное поведение тем же механизмом** — код объявления стиля рамки не менялся. Если эффект пропал у оператора, вероятнее:
   - регрессия отрисовки связки **`Table` + `.block()`** в текущей связке **ratatui 0.26.3** / бэкенда терминала;
   - или среда (тема, `NO_COLOR`, 16-color palette), где жёлтый на границе не различим.
3. **Рекомендация вместо `git revert`:** рендер «рамки» и «таблицы» развести: сначала `f.render_widget(Block, area)`, затем `Table` **во `block.inner(area)`** без `.block()` на таблице — стандартный обходной путь, чтобы стиль границы не зависел от внутренней композиции `Table`.

## Принятый follow-up (код)

В том же репозитории применена правка **двухпроходной отрисовки** блока и таблицы для Owner и Receivers в `tui_loop.rs` (см. текущий `main`).

## Команды для самопроверки истории

```text
git blame -L 978,996 crates/pwm-tui/src/tui_loop.rs
git log --oneline -S border_style -- crates/pwm-tui/src/tui_loop.rs
git show 104dce3 -- Cargo.toml
```

---

## Дополнение от оператора (2026-05-12): разные способы запуска PowerShell

**Наблюдение:** одно и то же TUI ведёт себя по-разному:

| Запуск | Фон | Поведение рамки / фокуса |
|--------|-----|---------------------------|
| PowerShell **поверх `cmd.exe`** | Чёрный | Ожидаемое (ранее) |
| PowerShell **из меню «Пуск»** (прямой запуск) | **Синий** (как у оболочки/профиля) | Деградация UX |

**Гипотеза для углублённого разбора (`pwm-review`, при необходимости совместно с `pwm-debug`, `verbosity-focus: tui`):**

1. **Не один и тот же хост терминала:** прямой запуск с Пуска часто открывает **Windows Terminal** с профилем PowerShell (фон/палитра из `settings.json`), а цепочка `cmd → powershell` может оказаться в **консоли conhost** или другом профиле WT — различаются **default background**, ANSI/VT обработка и иногда **цветовые пары** (жёлтый на синем может «смываться» визуально или конфликтовать с темой).
2. **`Alternate Screen` / crossterm:** приложение само не задаёт «синий фон страницы» как цвет оболочки — но отрисовка в альтернативном буфере **наследует** или **смешивается** с темой хоста иначе, чем в другом хосте; нужна сверка: тот ли процесс, тот ли `TERM`/backend.
3. **Не смешивать с «коммитом-виновником»:** пока нет корреляции с конкретным git-коммитом при фиксированном двоичнике — меняется **среда**, а не обязательно код.

**Чеклист для `pwm-review` (документировать, не править Rust без мандата `pwm-coding`):**

- Зафиксировать для обоих сценариев: имя хоста (WT / conhost), версия, активный профиль, флаги (`NO_COLOR`, тема).
- Сравнить скрин/запись или дамп: видны ли вообще границы блоков и ANSI yellow (`38;5;11` / `33`).
- Если различие только в теме — рекомендации в **операторской** документации (`docs/pwm-tui.md`): явный шаг «проверять в известном профиле» или отключение стиля оболочки для TUX-сессии.
- Любой кодовый hardening (принудительный фон буфера, отдельная цветовая схема) — отдельный тикет → **`pwm-coding`** после выводов ревью.

**Роль оркестратора:** не глубокий форензик TUI самостоятельно; держать конвейер, направить **`pwm-review`** на этот файл и при необходимости **`pwm-debug`** с узким `verbosity-focus`, **`pwm-coding`** — только после согласованного решения.

---

## pwm-review: Implementation Spec (2026-05-12)

### 1. Scope Recap
Stabilize TUI appearance (specifically the active panel's yellow border) across different terminal host contexts (e.g., Windows Terminal vs legacy conhost) by ensuring a consistent background color and robust terminal state management.

### 2. Root-Cause Hypothesis
The visual regression (yellow border disappearing or clashing) is caused by the TUI inheriting the terminal host's default background color. 
- When PowerShell is launched via `cmd.exe`, it runs in legacy `conhost` with a default black background.
- When launched directly from the Start Menu, it often opens in Windows Terminal (WT) with the default PowerShell profile, which uses a blue background (`#012456` or similar). 
- `ratatui`'s default `Style::default()` leaves the background transparent (inheriting the host's background). Yellow (`Color::Yellow`) on a bright/themed blue background lacks contrast, making the active border invisible or washed out.

### 3. Concrete Fix Proposal for `pwm-coding`

**A. Force Black Background (Ratatui level)**
Instead of relying on terminal defaults or `crossterm` escape sequences that might not cover all cleared areas reliably, force a black background for the entire TUI surface during the render pass.
- **Implementation:** At the very beginning of the `term.draw(|f| { ... })` closure in `tui_loop.rs`, render a full-screen block with a black background:
  `f.render_widget(Block::default().style(Style::default().bg(Color::Black)), f.size());`
- **Trade-offs:** Forces a dark theme. Users with light themes or specific accessibility needs (e.g., high contrast) might find the sudden black screen jarring. However, OLED screens benefit from true black, and it guarantees our yellow borders are visible.

**B. Terminal State Safety (Drop Guard)**
Currently, `tui_loop.rs` manually calls `disable_raw_mode()` and `LeaveAlternateScreen` at the end of `run()`. If a panic occurs, the terminal is left in a broken state (raw mode active, no echo, alternate screen stuck).
- **Implementation:** Create a scoped `Drop` guard struct (e.g., `struct TerminalGuard`) at the start of `run()`. In its `Drop` implementation, execute `disable_raw_mode()` and `LeaveAlternateScreen`. Remove the manual cleanup at the end of the function.

**C. Environment Opt-Out**
Provide an escape hatch for users who *want* their terminal theme to show through or have accessibility requirements.
- **Implementation:** Check for an environment variable, e.g., `PWM_TUI_INHERIT_HOST_COLORS=1`, at startup. If set, skip rendering the full-screen black block.

**D. Linux Notes**
- Analogous issues occur on Linux depending on the terminal emulator (e.g., `gnome-terminal` defaults to a dark theme, but `konsole` or `xterm` might have light or custom themes).
- `tmux` and `screen` can also interfere with background color clearing due to `BCE` (Background Color Erase) behavior. The Ratatui full-screen block approach bypasses `BCE` issues by explicitly painting every cell black, making it highly robust across Linux multiplexers.

### 4. Acceptance Criteria
`pwm-coding` must ensure the following manual test matrix passes:
1. **Windows Terminal (PowerShell profile):** Launch directly. Background must be black, active panel border must be clearly yellow.
2. **Legacy conhost (`cmd.exe` -> `powershell`):** Background must be black, border yellow.
3. **Linux (Ubuntu/Debian via WSL or native):** Test in `gnome-terminal` (or WT Ubuntu profile) and inside `tmux`. Background black, border yellow.
4. **Opt-out:** Set `$env:PWM_TUI_INHERIT_HOST_COLORS=1` and launch in WT PowerShell. The blue background should show through.
5. **Panic Safety:** Temporarily insert a `panic!("test")` in the event loop. The terminal must cleanly exit raw mode and return to the primary screen.

### 5. Verdict
**Approve spec.** The proposed changes are isolated to the presentation layer and terminal lifecycle, with no impact on core cryptography or networking logic.

### 6. Participation / Token Estimate
- `pwm-review`: ~2000 tokens (read TUI loop, formulate spec).

---

## Эскалация: pwm-debug (2026-05-12, оператор)

**Состояние после `pwm-coding` (фон + guard):**

- Полноэкранный чёрный фон работает **во всех** проверенных способах запуска.
- **Активная рамка** панелей Owner / Receivers по-прежнему **не** меняет цвет при запуске из **выделенного PowerShell** (сценарий «PowerShell из Пуска» → как правило **Windows Terminal** с профилем PowerShell).
- **Статический жёлтый** в интерфейсе **виден** (оператор: «цветовые метки столбцов панелей все жёлтые» — т.е. отрисовка `fg(Yellow)` на текст/элементы консистентна с ожиданием; проблема узкая: **`border_style` / рамка Block** для активной панели).

**Гипотезы для `pwm-debug` (`verbosity-focus: tui:border`):**

1. Различие ANSI/SGR между путём «цвет глифа/ячейки» и путём «цвет линий рамки» в связке **ratatui 0.26.3 + crossterm + WT** (профиль/«цвета текста по умолчанию», intense, cursor themes).
2. Порядок композиции: `Table` во `inner` всё ещё перекрывает участок границы (или BCE/`Erase` в альтернативном буфере WT).
3. `border_style` в этой версии ratatui не попадает в буфер в определённых конфигурациях (баг или известное ограничение — проверить по исходникам/ issue, **без** догадок в проде).

**Тикет оркестратора:** `tasks/20260512-tui-wt-border-debug.json`.

**Ожидаемый артефакт:** отчёт с воспроизведением, корневой причиной, ссылкой на файл/строки кода и рекомендацией для **`pwm-coding`** (обходной путь рендера рамки, смена цвета/модификатора, отдельный виджет границы и т.д.). Временная инструментация — только под флаг/`debug_assertions`, по умолчанию откатить.
