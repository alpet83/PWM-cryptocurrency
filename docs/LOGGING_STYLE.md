# pwmd logging style (Slice 18)

## Contract

- Base line format: `[HH:MM:SS.mmm] #TAG: event | k1=v1 k2=v2 ...`.
- Tags: `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR` (uppercase with `#` prefix).
- Stream routing: `WARN`/`ERROR` -> `stderr`, other levels -> `stdout`.
- File sink keeps plain text (no ANSI), safe for parsers.

## Color policy

- Console colors are enabled only for TTY (`auto` mode).
- Non-TTY or `NO_COLOR` should produce plain output.
- Numeric values are highlighted with bright purple.
- String values are highlighted with light green.
- Structures like JSON dump are highlighted with white. 
- `#ERROR` uses bright red; `#WARN` uses dark red.
- Regular information messages text uses light blue (less focus).
- Matter/stage messages text uses yellow (more focus).

## Defaults

- Default verbosity for current build: `DEBUG` (when `RUST_LOG` is not set).
- Default file sink path pattern: `logs/{date}/{log_name}-{node_id}-{time}.log`.
- Template token `~UT` is supported as UTC wall-clock `HH:MM:SS.mmm` (for file names/layouts that need readable time).
- Template placeholder `{node_id}` resolves from runtime identity (`--node-id` effective value) and is sanitized for filesystem-safe file names.
- File sink rotation is size-based with retention cap:
  - `--log-rotate-size-mb` (default `32`)
  - `--log-rotate-max-files` (default `7`)

## Debug tx inclusion event

At `DEBUG` level, validator emits one event per included tx and affected account:

- event name: `tx_included`
- fields:
  - `height`
  - `tx_kind`
  - `tx_id`
  - `addr`
  - `bal_before`
  - `bal_after`
  - `bal_delta`
