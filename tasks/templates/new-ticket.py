#!/usr/bin/env python3
"""
Создаёт тикет из шаблона с JSONL-override на stdin.

Каждая строка stdin — JSON-объект, патчит шаблон (глубокий merge для dict,
замена для скалярных значений и списков). Последняя строка может содержать
все нужные поля за один вызов.

Использование:
  echo '{"id":"20260626-my-task","title":"Заголовок","brief":"..."}' \\
    | python tasks/templates/new-ticket.py coding

  # Несколько строк — применяются последовательно:
  printf '{"branch_id":"mvp-v8"}\\n{"id":"20260626-x","title":"X"}\\n' \\
    | python tasks/templates/new-ticket.py review

  # Без stdin — создаёт заготовку с плейсхолдерами для ручного редактирования:
  python tasks/templates/new-ticket.py coding

Результат кладётся в .cqds/team-tasks/<id>.json (staging для досмотра).
share_ticket перекладывает в queue/ при активации.
Если id содержит плейсхолдер YYYYMMDD-<slug> — файл НЕ создаётся (нужно заполнить).
"""

import json
import sys
import select
from datetime import date
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
TEMPLATES_DIR = Path(__file__).resolve().parent
STAGING_DIR = REPO_ROOT / ".cqds" / "team-tasks"


def deep_merge(base: dict, patch: dict) -> dict:
    result = dict(base)
    for k, v in patch.items():
        if isinstance(v, dict) and isinstance(result.get(k), dict):
            result[k] = deep_merge(result[k], v)
        else:
            result[k] = v
    return result


def load_template(kind: str) -> dict:
    path = TEMPLATES_DIR / f"{kind}-ticket.template.json"
    if not path.exists():
        print(f"ERROR: template not found: {path}", file=sys.stderr)
        sys.exit(1)
    return json.loads(path.read_text(encoding="utf-8"))


def read_stdin_overrides() -> list[dict]:
    overrides = []
    # Читаем stdin только если туда что-то передали (не TTY)
    if not sys.stdin.isatty():
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                overrides.append(json.loads(line))
            except json.JSONDecodeError as e:
                print(f"ERROR: bad JSON on stdin: {e}", file=sys.stderr)
                sys.exit(1)
    return overrides


def main():
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        print(__doc__)
        sys.exit(0)

    kind = sys.argv[1].lower()
    if kind not in ("coding", "review"):
        print(f"ERROR: kind must be 'coding' or 'review', got '{kind}'", file=sys.stderr)
        sys.exit(1)

    ticket = load_template(kind)

    # Применяем аргументы командной строки как быстрые overrides
    # python new-ticket.py coding <id> "<title>"
    if len(sys.argv) >= 3:
        ticket["id"] = sys.argv[2]
    if len(sys.argv) >= 4:
        ticket["title"] = sys.argv[3]

    # Применяем JSONL-overrides со stdin
    for patch in read_stdin_overrides():
        ticket = deep_merge(ticket, patch)

    # Заполняем planned_for если остался плейсхолдер
    if ticket.get("planned_for", "").startswith("YYYY"):
        ticket["planned_for"] = date.today().strftime("%Y-%m-%d")

    ticket_id = ticket.get("id", "")

    # Не создаём файл если id не заполнен
    if "YYYYMMDD" in ticket_id or "<slug>" in ticket_id:
        print("Ticket preview (id not set, file NOT written):")
        print(json.dumps(ticket, ensure_ascii=False, indent=2))
        print("\nSet 'id' via stdin or argv to write the file.", file=sys.stderr)
        sys.exit(0)

    STAGING_DIR.mkdir(parents=True, exist_ok=True)
    out_path = STAGING_DIR / f"{ticket_id}.json"

    if out_path.exists():
        print(f"ERROR: file already exists: {out_path}", file=sys.stderr)
        sys.exit(1)

    out_path.write_text(
        json.dumps(ticket, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"Created: {out_path}")
    print(f"Next: review/edit, then share_ticket {ticket_id} project_id=5")


if __name__ == "__main__":
    main()
