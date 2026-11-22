"""Draws the editor's icon, three twinkling yellow five-pointed stars, into a Windows .ico.

The icon is generated rather than drawn by hand so that every size is
rendered from the same shapes. Run it again after changing them:

    python tools/make_icon.py [kss-editor/assets/kss-editor.ico] [--preview out.png]

Needs only the standard library: each size is supersampled in pure Python
and stored in the .ico as a PNG image (supported since Windows Vista).
"""

import math
import struct
import sys
import zlib
from pathlib import Path

SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)

# Each star: centre x, centre y, radius (tip to centre), all as fractions of
# the icon's side, and a tilt in degrees. Later stars are drawn over earlier
# ones.
STARS = (
    (0.40, 0.59, 0.37, -8),  # the big one, lower left
    (0.75, 0.25, 0.21, 12),  # upper right
    (0.81, 0.80, 0.15, -14),  # small, lower right
)

# The five-pointed star's inner corners, as a fraction of its radius. A
# geometric star is 0.38; a little more makes it chubbier.
INNER = 0.46

CORE = (255, 251, 214)  # near white, at the centre of a star
FILL = (255, 208, 32)  # gold yellow, towards the tips
OUTLINE = (176, 98, 0)  # dark amber, so the stars read on light backgrounds


def polygon(cx, cy, radius, inner, tilt):
    """The ten corners of a five-pointed star, first tip pointing up."""
    corners = []
    for i in range(10):
        angle = math.radians(tilt + 36 * i)
        r = radius if i % 2 == 0 else inner
        corners.append((cx + r * math.sin(angle), cy - r * math.cos(angle)))
    return corners


def inside(x, y, corners):
    """Whether (x, y) is inside the polygon `corners` (even-odd rule)."""
    hit = False
    j = len(corners) - 1
    for i, (xi, yi) in enumerate(corners):
        xj, yj = corners[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            hit = not hit
        j = i
    return hit


def shapes(outline):
    """Per star: its centre, radius, fill polygon and outline polygon."""
    out = []
    for cx, cy, radius, tilt in STARS:
        fill = polygon(cx, cy, radius, radius * INNER, tilt)
        # The tips are sharp, so they grow more than the inner corners for
        # the outline to look even.
        rim = polygon(cx, cy, radius + 2.2 * outline, radius * INNER + 1.2 * outline, tilt)
        out.append((cx, cy, radius, fill, rim))
    return out


def colour_at(x, y, stars):
    """The colour of the topmost star at (x, y), or None outside them all."""
    found = None
    for cx, cy, radius, fill, rim in stars:
        if inside(x, y, fill):
            t = min(1.0, math.hypot(x - cx, y - cy) / (radius * INNER) * 0.9)
            found = tuple(round(c + (f - c) * t) for c, f in zip(CORE, FILL))
        elif inside(x, y, rim):
            found = OUTLINE
    return found


def render(size):
    """RGBA rows of the icon at `size` x `size` pixels."""
    samples = 4 if size >= 64 else 6
    # About one pixel of outline at small sizes, thinner in proportion above.
    outline = max(0.8 / size, 0.012)
    stars = shapes(outline)
    step = 1.0 / (size * samples)
    rows = []
    for py in range(size):
        row = bytearray()
        for px in range(size):
            r = g = b = a = 0
            for sy in range(samples):
                y = (py * samples + sy + 0.5) * step
                for sx in range(samples):
                    x = (px * samples + sx + 0.5) * step
                    colour = colour_at(x, y, stars)
                    if colour is not None:
                        r += colour[0]
                        g += colour[1]
                        b += colour[2]
                        a += 1
            if a:
                row += bytes((r // a, g // a, b // a, a * 255 // (samples * samples)))
            else:
                row += bytes(4)
        rows.append(bytes(row))
    return rows


def png(size, rows):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    raw = b"".join(b"\x00" + row for row in rows)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def ico(images):
    """An .ico holding `images`, a list of (size, png bytes)."""
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for size, data in images:
        side = 0 if size >= 256 else size
        out += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return out + b"".join(data for _, data in images)


def main(args):
    preview = None
    if "--preview" in args:
        at = args.index("--preview")
        preview = Path(args[at + 1])
        del args[at : at + 2]
    target = Path(args[0]) if args else Path("kss-editor/assets/kss-editor.ico")
    images = [(size, png(size, render(size))) for size in SIZES]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(ico(images))
    print(f"wrote {target} ({', '.join(str(s) for s in SIZES)} px)")
    if preview:
        preview.write_bytes(images[-1][1])
        print(f"wrote {preview}")


if __name__ == "__main__":
    main(sys.argv[1:])
