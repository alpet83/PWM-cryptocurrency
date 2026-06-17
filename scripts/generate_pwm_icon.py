#!/usr/bin/env python3
"""Regenerate assets/branding/pwm.ico from pwm-icon.png.

Dark/near-black pixels around the orange logo are made fully transparent
(UI screenshot backgrounds, chart chrome, etc.).
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[1]
SRC = REPO / "assets" / "branding" / "pwm-icon.png"
OUT = REPO / "assets" / "branding" / "pwm.ico"

# Pixels darker than this (max RGB channel) become transparent.
DARK_THRESHOLD = 48

# Orange logo core: keep even if slightly dark edge anti-alias.
# Require at least one channel clearly in the orange range.
def is_logo_pixel(r: int, g: int, b: int) -> bool:
    return r >= 160 and g >= 60 and b <= 120 and r > b + 40


def make_transparent(img: Image.Image) -> Image.Image:
    rgba = img.convert("RGBA")
    px = rgba.load()
    w, h = rgba.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            if is_logo_pixel(r, g, b):
                continue
            if max(r, g, b) <= DARK_THRESHOLD:
                px[x, y] = (r, g, b, 0)
    return rgba


def main() -> None:
    if not SRC.is_file():
        raise SystemExit(f"missing source: {SRC}")
    img = make_transparent(Image.open(SRC))
    sizes = [256, 128, 64, 48, 32, 16]
    icons = [img.resize((s, s), Image.Resampling.LANCZOS) for s in sizes]
    icons[0].save(
        OUT,
        format="ICO",
        sizes=[(s, s) for s in sizes],
        append_images=icons[1:],
    )
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
