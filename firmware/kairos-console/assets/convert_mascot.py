#!/usr/bin/env python3
"""Convert a mascot PNG to a PROGMEM RGB565 header, or draw a fallback sprite."""

from __future__ import annotations

import argparse
from pathlib import Path

WIDTH = 112
HEIGHT = 176


def rgb565(r: int, g: int, b: int) -> int:
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)


def fill(pixels: list[list[int]], x: int, y: int, w: int, h: int, color: int) -> None:
    for yy in range(max(0, y), min(HEIGHT, y + h)):
        row = pixels[yy]
        for xx in range(max(0, x), min(WIDTH, x + w)):
            row[xx] = color


def pixel(pixels: list[list[int]], x: int, y: int, color: int) -> None:
    if 0 <= x < WIDTH and 0 <= y < HEIGHT:
        pixels[y][x] = color


def draw_fallback() -> list[list[int]]:
    black = rgb565(0, 0, 0)
    ink = rgb565(10, 10, 14)
    hair = rgb565(18, 18, 22)
    hair_hi = rgb565(40, 210, 255)
    skin = rgb565(236, 196, 168)
    eye = rgb565(20, 22, 28)
    mask = rgb565(16, 16, 20)
    jacket = rgb565(22, 24, 30)
    jacket_hi = rgb565(36, 40, 48)
    neon = rgb565(20, 220, 255)
    shirt = rgb565(12, 12, 16)
    slash = rgb565(120, 128, 140)
    pants = rgb565(18, 20, 26)
    shoe = rgb565(8, 8, 10)
    sole = rgb565(230, 230, 234)
    buckle = rgb565(90, 96, 110)

    pixels = [[black for _ in range(WIDTH)] for _ in range(HEIGHT)]

    # Hair mass
    fill(pixels, 28, 8, 56, 38, hair)
    fill(pixels, 22, 18, 14, 28, hair)
    fill(pixels, 76, 16, 16, 26, hair)
    fill(pixels, 34, 4, 18, 10, hair)
    fill(pixels, 54, 2, 16, 10, hair)
    for x, y in ((26, 10), (40, 6), (58, 4), (74, 12), (82, 20)):
        fill(pixels, x, y, 6, 4, hair)
    for x, y in ((30, 14), (48, 8), (66, 10), (78, 22)):
        fill(pixels, x, y, 4, 3, hair_hi)

    # Head
    fill(pixels, 34, 28, 44, 36, skin)
    fill(pixels, 38, 24, 36, 8, skin)
    fill(pixels, 36, 58, 40, 8, skin)

    # Eyes and mole
    fill(pixels, 42, 40, 8, 8, eye)
    fill(pixels, 62, 40, 8, 8, eye)
    fill(pixels, 44, 42, 3, 3, rgb565(240, 240, 245))
    fill(pixels, 64, 42, 3, 3, rgb565(240, 240, 245))
    pixel(pixels, 72, 50, ink)

    # Mask
    fill(pixels, 36, 52, 40, 16, mask)
    fill(pixels, 34, 56, 4, 8, mask)
    fill(pixels, 74, 56, 6, 8, mask)
    fill(pixels, 40, 58, 28, 2, neon)
    fill(pixels, 70, 56, 5, 5, neon)
    fill(pixels, 71, 57, 3, 3, mask)

    # Torso / jacket
    fill(pixels, 30, 70, 52, 58, jacket)
    fill(pixels, 26, 76, 10, 46, jacket)
    fill(pixels, 76, 76, 12, 46, jacket)
    fill(pixels, 42, 78, 28, 26, shirt)
    fill(pixels, 50, 84, 14, 3, slash)
    fill(pixels, 52, 90, 10, 3, slash)
    fill(pixels, 28, 74, 56, 3, neon)
    fill(pixels, 26, 88, 8, 3, neon)
    fill(pixels, 80, 88, 8, 3, neon)
    fill(pixels, 30, 118, 52, 3, neon)
    fill(pixels, 22, 80, 6, 18, jacket_hi)
    fill(pixels, 84, 80, 6, 18, jacket_hi)
    fill(pixels, 24, 98, 6, 4, buckle)
    fill(pixels, 82, 98, 6, 4, buckle)

    # Arms in pockets
    fill(pixels, 18, 92, 12, 28, jacket)
    fill(pixels, 82, 92, 14, 28, jacket)
    fill(pixels, 20, 116, 10, 6, neon)
    fill(pixels, 84, 116, 10, 6, neon)

    # Pants and hanging straps
    fill(pixels, 34, 128, 44, 30, pants)
    fill(pixels, 32, 132, 12, 26, pants)
    fill(pixels, 68, 132, 12, 26, pants)
    fill(pixels, 38, 140, 8, 10, jacket_hi)
    fill(pixels, 66, 140, 8, 10, jacket_hi)
    fill(pixels, 36, 150, 6, 8, neon)
    fill(pixels, 70, 150, 6, 8, neon)
    fill(pixels, 48, 128, 16, 2, neon)

    # Shoes
    fill(pixels, 28, 158, 22, 12, shoe)
    fill(pixels, 62, 158, 22, 12, shoe)
    fill(pixels, 28, 168, 22, 4, sole)
    fill(pixels, 62, 168, 22, 4, sole)
    fill(pixels, 32, 160, 8, 4, neon)
    fill(pixels, 66, 160, 8, 4, neon)
    return pixels


def load_png(path: Path) -> list[list[int]]:
    from PIL import Image

    image = Image.open(path).convert("RGBA")
    image.thumbnail((WIDTH, HEIGHT), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (WIDTH, HEIGHT), (0, 0, 0, 255))
    x = (WIDTH - image.width) // 2
    y = max(0, HEIGHT - image.height - 24)
    canvas.paste(image, (x, y), image)
    pixels = []
    for yy in range(HEIGHT):
        row = []
        for xx in range(WIDTH):
            r, g, b, a = canvas.getpixel((xx, yy))
            if a < 16:
                row.append(rgb565(0, 0, 0))
            else:
                row.append(rgb565(r, g, b))
        pixels.append(row)
    return pixels


def write_header(pixels: list[list[int]], dest: Path) -> None:
    values = [f"0x{color:04X}" for row in pixels for color in row]
    lines = [
        "#pragma once",
        "",
        "#include <cstdint>",
        "",
        "#ifndef PROGMEM",
        "#define PROGMEM",
        "#endif",
        "",
        "namespace kairos::assets {",
        "",
        f"constexpr int kMascotWidth = {WIDTH};",
        f"constexpr int kMascotHeight = {HEIGHT};",
        "",
        "const std::uint16_t kMascotBase[] PROGMEM = {",
    ]
    for index in range(0, len(values), 12):
        chunk = ", ".join(values[index : index + 12])
        lines.append(f"    {chunk},")
    lines.extend(
        [
            "};",
            "",
            "}  // namespace kairos::assets",
            "",
        ]
    )
    dest.write_text("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path(__file__).with_name("mascot_source.png"))
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "include" / "assets" / "mascot_base.hpp",
    )
    args = parser.parse_args()
    if args.source.exists():
        try:
            pixels = load_png(args.source)
        except ImportError:
            pixels = draw_fallback()
    else:
        pixels = draw_fallback()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    write_header(pixels, args.output)


if __name__ == "__main__":
    main()
