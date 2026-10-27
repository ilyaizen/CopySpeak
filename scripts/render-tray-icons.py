#!/usr/bin/env python3
"""Derive colored tray artwork from CopySpeak's original RGBA icon.

Requires Pillow. Hue rotation retains each pixel's saturation, brightness and alpha.
"""

import colorsys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "src-tauri/icons/icon.png"
OUTPUT = ROOT / "src-tauri/icons/tray"


def render(hue_shift: float, destination: Path) -> None:
    image = Image.open(SOURCE).convert("RGBA")
    pixels = []
    for red, green, blue, alpha in image.getdata():
        hue, saturation, value = colorsys.rgb_to_hsv(red / 255, green / 255, blue / 255)
        r, g, b = colorsys.hsv_to_rgb((hue + hue_shift) % 1, saturation, value)
        pixels.append((round(r * 255), round(g * 255), round(b * 255), alpha))
    image.putdata(pixels)
    image.save(destination)


if __name__ == "__main__":
    OUTPUT.mkdir(parents=True, exist_ok=True)
    render(-0.13, OUTPUT / "speaking.png")
    render(-0.45, OUTPUT / "paused.png")
