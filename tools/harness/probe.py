"""Tracing what the game holds in memory back to the ROM.

    python tools/harness/probe.py vram      # where room 1's characters come from
    python tools/harness/probe.py stream 0x242BB2 0x800
                                            # find the compressed stream around an offset

The idea behind both: the game's memory is the answer sheet. Whatever is in
VRAM or RAM came from the ROM, so search the ROM (raw, and decompressed) for
the exact bytes. An exact match of 32 bytes is not a coincidence.
"""

import sys

from kss import Rom, decompress
from session import enter


def vram_sources(first=0, last=0x400):
    """Prints, for each run of VRAM characters, the resource it was loaded from.

    Every entry of the resource table is decompressed (raw entries fail and
    are skipped), every 32-byte character is indexed, and each VRAM character
    is looked up. Runs of consecutive characters from one resource are
    printed as one line.
    """
    rom = Rom()
    count = (0x3F0000 + rom.u16(0x3F0004) - rom.resource_table) // 3
    index = {}
    sizes = {}
    for resource in range(1, count):
        try:
            data = rom.unpack_resource(resource)
        except IndexError:
            continue  # not compressed (a palette), or not a stream at all
        sizes[resource] = len(data) // 32
        for character in range(len(data) // 32):
            index.setdefault(data[character * 32:character * 32 + 32], []).append((resource, character))
    vram = enter().vram()

    def source(character):
        tile = vram[character * 32:character * 32 + 32]
        return "blank" if tile == bytes(32) else index.get(tile)

    character = first
    while character < last:
        found = source(character)
        end = character
        if found is None or found == "blank":
            while end < last and source(end) == found:
                end += 1
            print("%03X-%03X  %s" % (character, end - 1, found or "not in any resource (animated, or loaded from elsewhere)"))
        else:
            # Longest run that stays consecutive in one resource.
            best = (end, None)
            for (resource, at) in found:
                stop = character
                while stop < last and isinstance(source(stop), list) and (resource, at + stop - character) in source(stop):
                    stop += 1
                if stop > best[0] or best[1] is None:
                    best = (stop, (resource, at))
            end, (resource, at) = best
            print("%03X-%03X  resource %02X from its character %03X (%d characters in it)"
                  % (character, end - 1, resource, at, sizes[resource]))
        character = max(end, character + 1)


def find_stream(near, back=0x800, needle=None):
    """Finds where the compressed stream containing file offset `near` starts.

    Back-references are absolute, so a stream decoded from the wrong start
    gives wrong bytes or fails. Every start in the `back` bytes before `near`
    is tried; the ones that decode through `near` (and contain `needle`, when
    given) are printed. The right one usually has the earliest start and a
    length that makes sense (a multiple of 32 for characters, plus a header).
    """
    rom = Rom()
    for start in range(near - back, near):
        try:
            data, used = decompress(rom.data, start)
        except IndexError:
            continue
        if start + used <= near or len(data) < 0x100:
            continue
        if needle is not None and needle not in data:
            continue
        print("start %06X  %5d bytes out, %5d in, first bytes %s" % (start, len(data), used, data[:8].hex(" ")))


if __name__ == "__main__":
    arguments = sys.argv[1:]
    if arguments[:1] == ["vram"]:
        vram_sources()
    elif arguments[:1] == ["stream"] and len(arguments) >= 2:
        find_stream(int(arguments[1], 0), int(arguments[2], 0) if len(arguments) > 2 else 0x800)
    else:
        sys.exit(__doc__)
