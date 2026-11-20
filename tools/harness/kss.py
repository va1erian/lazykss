"""Reading the ROM from Python: a small, independent copy of what kss-core does.

It exists so that an experiment does not depend on the code under test. When
this and kss-core disagree, one of them is wrong, which is worth knowing.

    from kss import Rom
    rom = Rom()
    room = rom.room(1)
    print(room.width, room.height, [hex(t) for t in room.tiles[:8]])

Formats: KSS_Level_Editor_Documentation.md.
"""

import struct

from emu import find_rom

MASTER = 0x3F0000


def decompress(data, start):
    """Inflates the HAL stream at `data[start]`. Returns `(output, bytes consumed)`."""
    out = bytearray()
    at = start
    while True:
        control = data[at]
        at += 1
        if control == 0xFF:
            return bytes(out), at - start
        if control & 0xE0 == 0xE0:
            command = (control >> 2) & 7
            length = ((control & 3) << 8 | data[at]) + 1
            at += 1
        else:
            command = control >> 5
            length = (control & 0x1F) + 1
        if command == 0:
            out += data[at:at + length]
            at += length
        elif command == 1:
            out += bytes([data[at]]) * length
            at += 1
        elif command == 2:
            out += data[at:at + 2] * length
            at += 2
        elif command == 3:
            out += bytes((data[at] + i) & 0xFF for i in range(length))
            at += 1
        else:
            offset = data[at] << 8 | data[at + 1]
            at += 2
            for i in range(length):
                # Indexing raises on a bad reference: a stream decoded from the
                # wrong start usually fails here, which is the point.
                value = out[offset - i] if command == 6 else out[offset + i]
                out.append(int("{:08b}".format(value)[::-1], 2) if command == 5 else value)


class Room:
    """One room, parsed. See the documentation, sections 3 and 4."""

    def __init__(self, rom, index):
        data = rom.data
        self.index = index
        self.offset = rom.room_offset(index)
        section = self.offset + 58
        self.sections = []
        for _ in range(6):
            length = rom.u16(section)
            self.sections.append(data[section + 2:section + length])
            section += length
        self.width, self.height = rom.u16(section + 2), rom.u16(section + 4)
        self.blob_offset = section + 6
        self.head = data[self.offset:self.blob_offset]
        self.blob, self.packed_length = decompress(data, self.blob_offset)
        cells = self.width * self.height
        self.tiles = [self.blob[i] | self.blob[cells + i] << 8 for i in range(cells)]
        self.objects = []
        at = cells * 2
        while at + 6 <= len(self.blob) and self.blob[at:at + 2] != b"\xff\xff":
            variant, ident, x, y = struct.unpack_from("<BBHH", self.blob, at)
            self.objects.append((ident, variant, x, y))
            at += 6

    @property
    def gfx_header(self):
        return self.head[22:46]

    @property
    def object_slots(self):
        """The object graphics slot bytes, as the hex `patch_room slots` takes."""
        return self.head[42:46].hex()

    def tile(self, x, y):
        return self.tiles[y * self.width + x]


class Rom:
    def __init__(self, path=None):
        self.path = path or find_rom()
        self.data = open(self.path, "rb").read()
        self.room_table = MASTER + self.u16(MASTER)
        self.resource_table = MASTER + self.u16(MASTER + 2)
        self.room_count = (self.resource_table - self.room_table) // 4

    def u16(self, offset):
        return self.data[offset] | self.data[offset + 1] << 8

    def pointer(self, offset):
        """A 3-byte SNES long pointer, as a file offset."""
        return (self.data[offset] | self.data[offset + 1] << 8 | self.data[offset + 2] << 16) - 0xC00000

    def room_offset(self, index):
        return self.pointer(self.room_table + index * 4)

    def room(self, index):
        return Room(self, index)

    def resource(self, index):
        """File offset of entry `index` of the graphics/palette resource table."""
        return self.pointer(self.resource_table + index * 3)

    def unpack_resource(self, index):
        """A compressed resource, inflated. Palettes are raw: do not use this on them."""
        return decompress(self.data, self.resource(index))[0]


def tile_pixels(data, offset):
    """The 64 palette indices (0-15) of the 4bpp character at `data[offset]`, row-major."""
    pixels = bytearray(64)
    for y in range(8):
        planes = (data[offset + y * 2], data[offset + y * 2 + 1], data[offset + 16 + y * 2], data[offset + 17 + y * 2])
        for x in range(8):
            shift = 7 - x
            pixels[y * 8 + x] = sum(((plane >> shift) & 1) << bit for bit, plane in enumerate(planes))
    return pixels


def rgb(word):
    """A BGR555 colour word as an (r, g, b) tuple."""
    def channel(value):
        return (value & 31) << 3 | (value & 31) >> 2
    return (channel(word), channel(word >> 5), channel(word >> 10))
