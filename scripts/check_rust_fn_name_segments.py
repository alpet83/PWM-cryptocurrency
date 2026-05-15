#!/usr/bin/env python3
"""
Compatibility shim: delegates to ``check_entity_name_segments.py``.

Deprecated — use ``scripts/check_entity_name_segments.py`` directly (same CLI and JSON shape;
extra ``entity`` field per violation).
"""

from __future__ import annotations

import runpy
import sys
from pathlib import Path

if __name__ == "__main__":
    here = Path(__file__).resolve().parent
    target = here / "check_entity_name_segments.py"
    print(
        "warning: check_rust_fn_name_segments.py is deprecated; "
        "use scripts/check_entity_name_segments.py",
        file=sys.stderr,
    )
    runpy.run_path(str(target), run_name="__main__")
