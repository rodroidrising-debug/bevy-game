#!/usr/bin/env python3
"""Generate license-clean procedural textures (stdlib only, no PIL).
Writes warm menu backdrop + results backdrop into cooking-game/assets/images/.
"""
import math
import os
import random
import struct
import zlib

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                   "cooking-game", "assets", "images")
os.makedirs(OUT, exist_ok=True)


def write_png(path, w, h, px):
    def chunk(typ, data):
        c = struct.pack(">I", len(data)) + typ + data
        return c + struct.pack(">I", zlib.crc32(typ + data) & 0xFFFFFFFF)

    raw = bytearray()
    for y in range(h):
        raw.append(0)
        raw.extend(px[y])
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
           + chunk(b"IEND", b""))
    with open(path, "wb") as f:
        f.write(png)
    print("wrote", path, len(png), "bytes")


def value_noise(seed, w, h, cells):
    rnd = random.Random(seed)
    gx = [[rnd.random() for _ in range(cells + 1)] for _ in range(cells + 1)]

    def at(x, y):
        fx = x * cells / w
        fy = y * cells / h
        x0, y0 = int(fx), int(fy)
        tx, ty = fx - x0, fy - y0
        sx = tx * tx * (3 - 2 * tx)
        sy = ty * ty * (3 - 2 * ty)
        a = gx[y0][x0] * (1 - sx) + gx[y0][x0 + 1] * sx
        b = gx[y0 + 1][x0] * (1 - sx) + gx[y0 + 1][x0 + 1] * sx
        return a * (1 - sy) + b * sy

    return at


def menu_bg(w=640, h=360):
    n1 = value_noise(7, w, h, 6)
    n2 = value_noise(99, w, h, 18)
    px = []
    for y in range(h):
        row = bytearray()
        for x in range(w):
            t = y / max(h - 1, 1)
            # warm top glow -> dark bottom
            base = 0.16 + 0.30 * (1 - t) ** 1.6
            grain = (n1(x, y) - 0.5) * 0.10 + (n2(x, y) - 0.5) * 0.06
            # vignette
            dx = (x / w - 0.5) * 2
            dy = (y / h - 0.5) * 2
            vig = max(0.0, 1.0 - 0.55 * (dx * dx + dy * dy) ** 0.8)
            v = max(0.0, min(1.0, (base + grain) * vig + 0.02))
            r = int(255 * min(1.0, v * 1.25 + 0.03))
            g = int(255 * min(1.0, v * 0.85 + 0.015))
            b = int(255 * min(1.0, v * 0.62))
            row += bytes((r, g, b))
        px.append(bytes(row))
    return px


def results_bg(w=640, h=360):
    n1 = value_noise(21, w, h, 5)
    px = []
    for y in range(h):
        row = bytearray()
        for x in range(w):
            # golden radial glow center-top
            dx = (x / w - 0.5) * 2
            dy = (y / h - 0.35) * 2
            d = math.sqrt(dx * dx + dy * dy)
            glow = max(0.0, 1.0 - d) ** 2 * 0.5
            v = 0.10 + glow + (n1(x, y) - 0.5) * 0.08
            v = max(0.0, min(1.0, v))
            r = int(255 * min(1.0, v * 1.3 + 0.04))
            g = int(255 * min(1.0, v * 1.0 + 0.02))
            b = int(255 * min(1.0, v * 0.55))
            row += bytes((r, g, b))
        px.append(bytes(row))
    return px


write_png(os.path.join(OUT, "menu_bg.png"), 640, 360, menu_bg())
write_png(os.path.join(OUT, "results_bg.png"), 640, 360, results_bg())
