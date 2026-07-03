# pi-ollama-tool-fix

Pi extension: converts Ollama/Qwen **text JSON** tool calls into native `toolCall` blocks so pi executes MCP and built-in tools.

## Install

Project-local (auto-discovered when `pi -C` PWM repo):

```bash
pi install "file:P:/opt/docker/pwm-protocol/.pi/extensions/pi-ollama-tool-fix"
```

Or dev:

```bash
pi -e P:/opt/docker/pwm-protocol/.pi/extensions/pi-ollama-tool-fix/src/index.ts
```

## Config

`~/.pi/agent/settings.json` (recommended):

```json
{
  "defaultProvider": "default",
  "defaultModel": "qwen2.5-coder:14b"
}
```

Without these, bare `pi` falls back to built-in `google` — **pi-ollama-tool-fix** only wraps providers `ollama`/`default`, so tool JSON may print to stdout unchanged.

`~/.pi/agent/pi-ollama-tool-fix.json`:

```json
{
  "providers": ["ollama", "default"],
  "logging": true
}
```

Log: `~/.pi/agent/pi-ollama-tool-fix.log`

Command: `/ollama-tool-fix-status`

## Tests

```bash
cd .pi/extensions/pi-ollama-tool-fix
npm test
```

Requires Node with TypeScript import support (Node 22+ or pi runtime).

## Notes

- Replaces external `ollama_tool_fix_proxy.py` for pi sessions (no `:11435` baseUrl hack).
- Works with MCP tools (`gitbash_git_write_file`, `text_editor_session_*`) via dynamic tool allowlist from request context.
- **Disable `pi-json-tools`** when using this extension — both hook `message_end` and conflict on Qwen models.
- Companion/bench: use `--provider default` (see `~/.pi/agent/models.json`), keep `npm:pi-mcp-adapter` in `settings.json` for `--mcp-config`.
