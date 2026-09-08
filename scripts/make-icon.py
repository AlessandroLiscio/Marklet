#!/usr/bin/env python3
"""Generate the Marklet source icon as a 1024x1024 PNG, with no dependencies.

The mark is a document with a highlighted first line: at 16 px in a taskbar it
still reads as "text with a heading", which is what the app is. Anything more
detailed turns to mush at that size.

Run this only when the icon changes:

    python3 scripts/make-icon.py
    npx tauri icon src-tauri/icons/source.png

The second command generates every platform size into src-tauri/icons/.
"""

import struct
import zlib
from pathlib import Path

SIZE = 1024
BG = (0x1B, 0x2B, 0x44)       # deep slate blue
PAPER = (0xFA, 0xFA, 0xF8)    # near-white, matches --paper-000
ACCENT = (0x7F, 0xB3, 0xE8)   # the heading bar
RADIUS = 224                  # squircle-ish corner on the app tile


def rounded_rect(x0, y0, x1, y1, r):
    """Coverage test for a rounded rectangle, sampled at pixel centres."""

    def inside(px, py):
        cx = min(max(px, x0 + r), x1 - r)
        cy = min(max(py, y0 + r), y1 - r)
        dx, dy = px - cx, py - cy
        return dx * dx + dy * dy <= r * r

    return inside


def main() -> None:
    tile = rounded_rect(0, 0, SIZE, SIZE, RADIUS)

    # The page: a portrait sheet, optically centred (slightly high).
    page = rounded_rect(288, 236, 736, 800, 28)

    # Text lines. The first is the accent "heading", the rest are body.
    bars = []
    top = 330
    for i, (width, colour) in enumerate(
        [(272, ACCENT), (352, BG), (352, BG), (352, BG), (232, BG)]
    ):
        y = top + i * 92
        bars.append((rounded_rect(352, y, 352 + width, y + 44, 22), colour))

    rows = []
    # 2x2 supersampling: four samples per pixel is enough for edges this soft
    # and keeps the script fast enough to run inline.
    offsets = (0.25, 0.75)
    for y in range(SIZE):
        row = bytearray([0])  # PNG filter type 0 for this scanline
        for x in range(SIZE):
            acc = [0, 0, 0]
            hits = 0
            for oy in offsets:
                for ox in offsets:
                    px, py = x + ox, y + oy
                    if not tile(px, py):
                        continue
                    hits += 1
                    colour = BG
                    if page(px, py):
                        colour = PAPER
                        for bar, bar_colour in bars:
                            if bar(px, py):
                                colour = bar_colour
                                break
                    acc = [a + c for a, c in zip(acc, colour)]

            if hits == 0:
                row += bytes((0, 0, 0, 0))
                continue
            alpha = hits / 4
            colour = tuple(c // hits for c in acc)
            # Premultiplication is not used; PNG stores straight alpha.
            row += bytes(colour) + bytes((round(alpha * 255),))
        rows.append(bytes(row))

    raw = b"".join(rows)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )

    out = Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "source.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(png)
    print(f"{out} ({len(png):,} bytes)")


if __name__ == "__main__":
    main()
