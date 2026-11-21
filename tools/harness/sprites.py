"""Regenerates kss-core/src/sprite_table.rs from the running game.

    python tools/harness/sprites.py            # every room (0-318), about an hour
    python tools/harness/sprites.py 0 26       # another range of rooms (here Spring Breeze)
    python tools/harness/sprites.py --sheet    # also save a picture of every sprite

How it works, for each (object id, variant) the rooms use:

1. Patch room 1 so it holds only that object, near Kirby. The graphics it
   needs are supplied the cheapest way that works, tried in this order: the
   object graphics slots of the room the object came from; that room's whole
   graphics header; the room itself (room 1 becomes a copy of its pointer).
2. Run the game into the room and read OAM: every sprite that is not Kirby
   (character 0x60 and up) belongs to the object.
3. For each 8x8 character of those sprites, find the same 32 bytes in the
   decompressed object graphics packages (or the shared sheets). That gives a
   ROM address instead of pixels.
4. Find the object's frame definition in the ROM (5-byte records) to learn
   where its position is relative to its pieces.
5. Find where the palette row it uses is stored.

The table holds addresses and layout only. An object is left out when it
shows no sprite in any of the three set-ups (invisible markers, bosses that
need more than their room's graphics) or when a character cannot be traced to
the ROM.

Takes about three seconds per object that works in the first set-up and up to
thirty for one that works in none. See docs/emulator-harness.md.
"""

import collections
import functools
import subprocess
import sys

import numpy

from kss import Rom, decompress, rgb, tile_pixels
from session import REPO, enter, patch, work_dir

PACKAGE_TABLE = 0x101726
PACKAGE_TABLE_END = 0x1017F5
# Plain compressed character sheets, in addition to the packages.
SHARED_SHEETS = (0x212E30, 0x1BEE40, 0x1CC01B)
# Kirby's own characters are 0x00-0x5F; everything else is the object.
FIRST_OBJECT_CHARACTER = 0x60
TABLE = REPO / "kss-core" / "src" / "sprite_table.rs"

rom = Rom()
ROM = rom.data
ROM_ARRAY = numpy.frombuffer(ROM, dtype=numpy.uint8)


def signed(byte):
    return byte - 256 if byte > 127 else byte


def blank(tile):
    return tile.count(0) >= 30


# ---------------------------------------------------------------- sources

def load_sources():
    """Every stream object characters can come from: `(offset, tile start, data)`."""
    packages = {}
    for entry in range(PACKAGE_TABLE, PACKAGE_TABLE_END, 3):
        offset = rom.pointer(entry)
        if offset <= 0 or offset >= len(ROM) or offset in packages:
            continue
        try:
            data = decompress(ROM, offset)[0]
        except IndexError:
            continue
        # A package: a flag byte, then palettes of `0F, index, 15 colours`.
        start = 1
        while start + 32 <= len(data) and data[start] == 0x0F and data[start + 1] == data[2]:
            start += 32
        packages[offset] = (offset, start, data)
    sources = [packages[offset] for offset in sorted(packages)]
    sources += [(offset, 0, decompress(ROM, offset)[0]) for offset in SHARED_SHEETS]
    return sources


SOURCES = load_sources()
# 32 bytes of a character -> where it is: [(source index, character index)].
CHARACTERS = collections.defaultdict(list)
for index, (_, start, data) in enumerate(SOURCES):
    for character in range((len(data) - start) // 32):
        tile = data[start + character * 32:start + character * 32 + 32]
        if not blank(tile):
            CHARACTERS[tile].append((index, character))
# 30 bytes of palette colours 1-15 -> [(source index, palette index)].
PALETTES = collections.defaultdict(list)
for index, (_, start, data) in enumerate(SOURCES):
    if start == 161:
        for palette in range(5):
            PALETTES[data[3 + 32 * palette:33 + 32 * palette]].append((index, palette))


# ---------------------------------------------------------------- frames

def find_frame(pieces):
    """File offsets of frame definitions matching `pieces` (OAM order).

    A frame is a list of `flags, dx, dy, character, attributes` records; flag
    bit 0 = 16x16, bit 1 = last piece. The object may be drawn mirrored, so
    both orientations are tried, then again ignoring the palette bits.
    """
    count = len(pieces)
    for mirrored in (False, True):
        for mask in (0xFF, 0xF1):
            want = [(c & 0xFF, (a ^ 0x40 if mirrored else a) & mask, large) for (_, _, c, a, large) in pieces]
            found = []
            for offset in _starts(*want[0], mask):
                if offset + 5 * count > len(ROM) or not _records_match(offset, want, mask):
                    continue
                if all(_same_layout(pieces, offset, i, mirrored) for i in range(1, count)):
                    found.append(offset)
            if found:
                return found
    return []


@functools.lru_cache(maxsize=None)
def _starts(character, attributes, large, mask):
    """Offsets whose 5-byte record could be the first piece (cached: many calls repeat it)."""
    span = len(ROM_ARRAY) - 4
    match = ROM_ARRAY[3:span + 3] == character
    match &= (ROM_ARRAY[4:span + 4] & mask) == attributes
    match &= (ROM_ARRAY[:span] & 1) == large
    return [int(offset) for offset in numpy.nonzero(match)[0]]


def _records_match(offset, want, mask):
    """Whether the records at `offset` hold `want`, and only the last one ends the frame."""
    count = len(want)
    for i, (character, attributes, large) in enumerate(want):
        record = ROM[offset + i * 5:offset + i * 5 + 5]
        if record[3] != character or record[4] & mask != attributes or record[0] & 1 != large:
            return False
    return bool(ROM[offset + (count - 1) * 5] & 2) and not any(ROM[offset + i * 5] & 2 for i in range(count - 1))


def _same_layout(pieces, offset, i, mirrored):
    dx = signed(ROM[offset + i * 5 + 1]) - signed(ROM[offset + 1])
    dy = signed(ROM[offset + i * 5 + 2]) - signed(ROM[offset + 2])
    on_screen_x = pieces[i][0] - pieces[0][0]
    on_screen_y = pieces[i][1] - pieces[0][1]
    if mirrored:
        first, this = (16 if pieces[0][4] else 8), (16 if pieces[i][4] else 8)
        on_screen_x = -(on_screen_x + this - first)
    return (dx, dy) == (on_screen_x, on_screen_y)


def origin_of(pieces):
    """The object's position on screen: from its frame definition when one is
    found for the longest run of pieces, else the centre of the pieces."""
    for count in range(len(pieces), 0, -1):
        if count == 1 and len(pieces) > 1:
            break
        for first in range(len(pieces) - count + 1):
            frames = find_frame(pieces[first:first + count])
            if frames:
                offset = frames[0]
                return (pieces[first][0] - signed(ROM[offset + 1]), pieces[first][1] - signed(ROM[offset + 2]))
    left = min(p[0] for p in pieces)
    right = max(p[0] + (16 if p[4] else 8) for p in pieces)
    top = min(p[1] for p in pieces)
    bottom = max(p[1] + (16 if p[4] else 8) for p in pieces)
    return ((left + right) // 2, (top + bottom) // 2)


# ---------------------------------------------------------------- capture

def objects_in(rooms):
    """`{(id, variant): room}` for the first room each appears in."""
    found = {}
    for index in rooms:
        for ident, variant, _, _ in rom.room(index).objects:
            found.setdefault((ident, variant), index)
    return found


def own_room_rom(index):
    """Path of a copy of the ROM whose room 1 is room `index` (pointer copied)."""
    data = bytearray(ROM)
    entry = rom.room_table + 4 * index
    data[rom.room_table + 4:rom.room_table + 8] = ROM[entry:entry + 4]
    path = work_dir() / "own-room.sfc"
    path.write_bytes(data)
    return path


def placement(room, tier):
    """How to make the object appear: the graphics it needs, three ways.

    1. The room's four object graphics slots, in Green Greens room 1.
    2. The room's whole graphics header and fifth slot (sub-game sets).
    3. The room itself: room 1 becomes a copy of the room's pointer entry.
    """
    head = rom.room(room).head
    if tier == 0:
        return {"edits": ["slots", head[42:46].hex()]}
    if tier == 1:
        return {"edits": ["raw", "22", head[22:47].hex()]}
    return {"edits": [], "base": own_room_rom(room)}


TIERS = ("slots", "graphics header", "own room")


def capture(ident, variant, room, tier):
    """Places the object alone in room 1 and returns what the game drew."""
    how = placement(room, tier)
    object_edit = ["object", hex(ident), hex(variant), "110", "100"]
    patched = patch(1, "clear-objects", *how["edits"], *object_edit, base=how.get("base"))
    e = enter(patched)
    pieces = [sprite for sprite in e.oam() if sprite[2] >= FIRST_OBJECT_CHARACTER]
    return pieces, e.vram(), e.palette()


def trace(ident, variant, room):
    """The table entry for an object and the tier that worked, or `(None, why)`."""
    why = ""
    for tier, name in enumerate(TIERS):
        pieces, vram, palette = capture(ident, variant, room, tier)
        if not pieces:
            why = "no sprite in room 1 (from room %d)" % room
            continue
        entry, reason = resolve(pieces, vram, palette)
        if entry is not None:
            return entry, name
        why = reason
    return None, why


def resolve(pieces, vram, palette):
    """The table entry for an object, or `(None, why)`.

    Each entry piece is one 8x8 character:
    `(dx, dy, flip bits, source index, character index, palette)`.
    """
    cells = []
    votes = collections.Counter()
    for (x, y, character, attributes, large) in pieces:
        side = 2 if large else 1
        hflip, vflip = bool(attributes & 0x40), bool(attributes & 0x80)
        for ty in range(side):
            for tx in range(side):
                # A flipped 16x16 sprite also swaps its four characters.
                col = side - 1 - tx if hflip else tx
                row = side - 1 - ty if vflip else ty
                index = (character + col + 16 * row) & 0x1FF
                tile = vram[(0x600 + index) * 32:(0x600 + index) * 32 + 32]
                cells.append((x + tx * 8, y + ty * 8, hflip, vflip, (attributes >> 1) & 7, tile))
                for source, _ in CHARACTERS.get(tile, []):
                    votes[source] += 1
    if any(not blank(cell[5]) and not CHARACTERS.get(cell[5]) for cell in cells):
        # Checked before the slow frame search: most failures end here.
        return None, "a character is not in any known source"
    origin = origin_of(pieces)
    entry = []
    for (x, y, hflip, vflip, row, tile) in cells:
        if blank(tile):
            continue
        dx, dy = x - origin[0], y - origin[1]
        candidates = CHARACTERS[tile]
        # The same character can be in several packages: prefer the one most
        # of this object's characters are in.
        source, character = max(candidates, key=lambda c: votes[c[0]])
        colours = b"".join(w.to_bytes(2, "little") for w in palette[128 + row * 16 + 1:128 + row * 16 + 16])
        if SOURCES[source][1] == 161:
            own = [i for (s, i) in PALETTES.get(colours, []) if s == source]
            # Palette 4 is the hit flash, caught when the object has just
            # spawned: use the normal palette instead.
            where = ("package", source, own[0] if own and own[0] != 4 else 0)
        else:
            raw = ROM.find(colours)
            if raw >= 0:
                where = ("raw", raw, 0)
            elif PALETTES.get(colours):
                where = ("package",) + PALETTES[colours][0]
            else:
                return None, "its palette is not in the ROM as shown"
        entry.append((dx, dy, int(hflip) | int(vflip) << 1, source, character, where))
    return (entry, "") if entry else (None, "nothing but blank characters")


# ---------------------------------------------------------------- output

HEADER = '''// @generated by tools/harness/sprites.py; do not edit by hand.
//! Generated table: where each object's graphics live in the
//! ROM and how its 8x8 characters are laid out. It holds addresses and
//! layout only; every pixel and colour is read from the user's ROM.
//!
//! Built by placing each object in Green Greens room 1 (given the graphics
//! of the room it comes from, or, failing that, made into a copy of that
//! room), running the game in an emulator, and tracing the characters it put
//! in OAM back to the compressed graphics packages they were loaded from.
//! Covers the objects of every sub-game that could be traced.
//! Regenerate with `python tools/harness/sprites.py`.

/// A compressed stream of 4bpp characters: file offset, and the byte offset
/// of character 0 in the decompressed data (161 for an object graphics
/// package, which starts with a flag byte and five 32-byte palettes).
pub(super) const SOURCES: &[(u32, u16)] = &[
'''

MIDDLE = '''];

/// Where 15 BGR555 colours (palette entries 1-15) are stored.
#[derive(Clone, Copy)]
pub(super) enum Colors {
    /// Palette `index` (0-4) of the package at `SOURCES[source]`.
    Package { source: u8, index: u8 },
    /// Uncompressed, at this file offset.
    Raw(u32),
}

pub(super) const PALETTES: &[Colors] = &[
'''

TAIL = '''];

/// One 8x8 character of an object: offset from the object's position,
/// flip bits (1 = horizontal, 2 = vertical), source, character, palette.
pub(super) type Piece = (i16, i16, u8, u8, u16, u8);

/// `(object id, variant, pieces)`, front piece first, sorted by id then
/// variant.
#[rustfmt::skip]
pub(super) const OBJECTS: &[(u8, u8, &[Piece])] = &[
'''


def write_table(table):
    used = sorted({piece[3] for entry in table.values() for piece in entry})
    source_number = {source: number for number, source in enumerate(used)}
    palettes = sorted({piece[5] for entry in table.values() for piece in entry})
    palette_number = {where: number for number, where in enumerate(palettes)}
    out = [HEADER]
    for source in used:
        out.append("    (0x%06X, %d),\n" % (SOURCES[source][0], SOURCES[source][1]))
    out.append(MIDDLE)
    for where in palettes:
        if where[0] == "package":
            out.append("    Colors::Package { source: %d, index: %d },\n" % (source_number[where[1]], where[2]))
        else:
            out.append("    Colors::Raw(0x%06X),\n" % where[1])
    out.append(TAIL)
    for (ident, variant) in sorted(table):
        out.append("    (0x%02X, 0x%02X, &[\n" % (ident, variant))
        line = "       "
        for (dx, dy, flip, source, character, where) in table[(ident, variant)]:
            item = " (%d, %d, %d, %d, %d, %d)," % (dx, dy, flip, source_number[source], character, palette_number[where])
            if len(line) + len(item) > 110:
                out.append(line + "\n")
                line = "       "
            line += item
        out.append(line + "\n    ]),\n")
    out.append("];\n")
    TABLE.write_text("".join(out), encoding="utf-8", newline="\n")
    subprocess.run(["rustfmt", "--edition", "2024", str(TABLE)], check=True)


def save_sheet(table, path):
    """A picture of every object in the table, drawn from the ROM like kss-core does."""
    from PIL import Image, ImageDraw
    sheet = Image.new("RGB", (1500, 6000), (40, 40, 60))
    draw = ImageDraw.Draw(sheet)
    x = y = 4
    row_height = 0
    for key in sorted(table):
        entry = table[key]
        left, top = min(p[0] for p in entry), min(p[1] for p in entry)
        width, height = max(p[0] for p in entry) + 8 - left, max(p[1] for p in entry) + 8 - top
        image = Image.new("RGBA", (width, height), (0, 0, 0, 0))
        for (dx, dy, flip, source, character, where) in reversed(entry):
            _, start, data = SOURCES[source]
            pixels = tile_pixels(data, start + character * 32)
            if where[0] == "package":
                raw = SOURCES[where[1]][2][3 + 32 * where[2]:33 + 32 * where[2]]
            else:
                raw = ROM[where[1]:where[1] + 30]
            colours = [(0, 0, 0)] + [rgb(raw[i * 2] | raw[i * 2 + 1] << 8) for i in range(15)]
            for py in range(8):
                for px in range(8):
                    value = pixels[(7 - py if flip & 2 else py) * 8 + (7 - px if flip & 1 else px)]
                    if value:
                        image.putpixel((dx - left + px, dy - top + py), colours[value] + (255,))
        cell = max(width, 40)
        if x + cell > 1490:
            x, y, row_height = 4, y + row_height + 16, 0
        draw.text((x, y), "%02X/%02X" % key, fill=(255, 255, 255))
        sheet.paste(image, (x, y + 11), image)
        x += cell + 6
        row_height = max(row_height, height + 11)
    sheet.crop((0, 0, 1500, y + row_height + 10)).save(path)


def main(arguments):
    want_sheet = "--sheet" in arguments
    numbers = [int(a) for a in arguments if a != "--sheet"]
    rooms = range(numbers[0], numbers[1] + 1) if len(numbers) == 2 else range(0, 319)
    table = {}
    wanted = objects_in(rooms)
    for (ident, variant), room in sorted(wanted.items()):
        entry, how = trace(ident, variant, room)
        if entry is None:
            print("%02X/%02X  left out: %s" % (ident, variant, how), flush=True)
            continue
        table[(ident, variant)] = entry
        print("%02X/%02X  %d characters (%s)" % (ident, variant, len(entry), how), flush=True)
    print("%d of %d objects resolved" % (len(table), len(wanted)))
    write_table(table)
    print("wrote", TABLE)
    if want_sheet:
        save_sheet(table, work_dir() / "sprites.png")
        print("wrote", work_dir() / "sprites.png")


if __name__ == "__main__":
    main(sys.argv[1:])
