"""Checks that the harness works on this machine. Run it first.

    python tools/harness/selfcheck.py

It boots the game, walks into Green Greens room 1 and compares what the
emulator holds with what the ROM says should be there. Every line must say
"ok"; a failure names what to look at. A screenshot is written to
target/harness/selfcheck.png: open it and check it shows Kirby standing on
grass under a blue sky.
"""

import sys

from kss import Rom
from session import enter, work_dir

failures = []


def check(name, ok, hint=""):
    print(("ok    " if ok else "FAIL  ") + name + ("" if ok else "  <- " + hint))
    if not ok:
        failures.append(name)


rom = Rom()
room = rom.room(1)
e = enter()
e.shot(work_dir() / "selfcheck.png")
wram, vram = e.wram(), e.vram()

check("WRAM is 128 KiB and VRAM 64 KiB", (len(wram), len(vram)) == (0x20000, 0x10000),
      "the core does not expose SNES memory the way bsnes does")

in_ram = [wram[0x10000 + i * 2] | wram[0x10001 + i * 2] << 8 for i in range(len(room.tiles))]
check("the room's tile map is at 7F:0000 and equals the ROM's", in_ram == room.tiles,
      "the inputs in session.py did not reach room 1: look at the screenshot")

characters = rom.unpack_resource(room.gfx_header[2])
check("foreground characters 1-0x200 in VRAM equal the ROM's", vram[32:0x200 * 32] == characters[32:0x200 * 32],
      "see the documentation, section 5")

palette = e.palette()
at = rom.resource(room.gfx_header[8])
count, first = rom.data[at], rom.data[at + 1]
expected = [rom.u16(at + 2 + i * 2) for i in range(count)]
check("the background palette is in the 7E:0500 mirror", palette[first:first + count] == expected,
      "the palette mirror moved, or the room is not loaded")

kirby = [sprite for sprite in e.oam() if sprite[2] < 0x60]
check("OAM holds Kirby: six pieces, one of them 16x16", len(kirby) == 6 and sum(s[4] for s in kirby) == 1,
      "OAM is not 0x20000 after VRAM in this core's savestate: see Emu.oam")

print("screenshot:", work_dir() / "selfcheck.png")
sys.exit(1 if failures else 0)
