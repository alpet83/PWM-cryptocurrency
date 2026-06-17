# Публикация MVP (публичное зеркало)

Каноника описана в **`docs/COMMIT_PROTOCOL.md`**: рантайм `P:\opt\docker\PWM-cryptocurrency` → зеркало `P:\GitHub\PWM-cryptocurrency` только через MCP **`git_safe_commit`**.

## Когда публиковать

**Публикация** (`dry_run` → `apply` → `commit` на зеркале) — **только на полном closeout версии MVP** (напр. V6-11, V5-8): sprint-final review PASS, owner sign-off, checklist/CHANGELOG/GLOSSARY закрыты.

**Между слайсами** оркестратор делает **локальные коммиты** в рантайме (`git_safe_commit` `mode=commit`, `public_repo=false`) **без** `dry_run`/`apply`. См. **`docs/AGENT_PROMPT_orchestrator.md`** § Git.

## Порядок closeout-публикации

1. **`git_repo_status`** на обоих деревьях (рантайм и зеркало).
2. **`git_safe_commit`** `mode=dry_run`, `repo_path` = рантайм → отчёт и токен `dry_run_token`.
3. Устранить **CRITICAL** из отчёта (обычно CRLF → LF по `.gitattributes`, исключить артефакты вроде `.tmp-test/` через `commit_prepare.toml`).
4. Повторить **`dry_run`** до приемлемых предупреждений.
5. **`git_safe_commit`** `mode=apply`, тот же `repo_path`, **`apply_token`** из последнего успешного dry-run, **`confirm=I_UNDERSTAND_AND_APPROVE`**.
6. **`git_safe_commit`** `mode=commit`, **`repo_path`** = `P:\GitHub\PWM-cryptocurrency`, **`public_repo=true`**, **`commit_message`**, тот же **`confirm`**.
7. **`git push`** на `origin` — только по явной просьбе оператора (протокол).

## Рекомендация на следующий план

Чтобы слайсы были видны в истории публичного репозитория: по возможности **один осмысленный коммит на слайс** уже в рантайме до `dry_run`, или узкие коммиты перед синком — тогда `git log` на зеркале читается без монолита.
