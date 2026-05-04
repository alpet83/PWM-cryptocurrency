## Sprint 14 / Slice 17 — remediation3 coding note

- Added custom `pwmd` formatter for console/file events with exact contract:
  `[HH:MM:SS.mmm] #TAG: event | k=v ...`.
- Preserved existing routing and sink behavior:
  `WARN/ERROR -> stderr`, other levels -> `stdout`; file sink remains plain/no-ANSI.
- Implemented TTY palette for console:
  - `#ERROR` bright red
  - `#WARN` dark red
  - numeric fragments bright purple
- Added numeric highlighter rules:
  - applies to message text and `k=v` values
  - skips timestamp segment and hash/id-like tokens (hex/base58/base64-like).
- Added `NO_COLOR` precedence: when set, ANSI is always disabled even in TTY and even with `always`.
- Added formatter unit tests for plain format contract, tag colors, numeric highlighting, hash/id exclusions, and `NO_COLOR`.
