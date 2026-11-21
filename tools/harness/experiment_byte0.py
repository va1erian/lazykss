"""What is byte 0 of the room block?

    python tools/harness/experiment_byte0.py

  question    Byte 0 of a room block is 00 in Spring Breeze and 01-0A
              elsewhere. Does the game read it in a way that is visible?
  hypothesis  It tags the sub-game (a group id). If the game uses it, a
              changed value changes something in room 1; if not, nothing.
  test        Room 1 with byte 0 set to each value, the same inputs for
              each (hold Right, then Y), against the unchanged control.
  evidence    The screenshot, WRAM and BW-RAM after the same frames; the
              number of differing bytes against the control, and a
              contact sheet. The unchanged ROM is run twice to show what
              plain emulator nondeterminism looks like (none is expected).

Result: nothing. For byte 0 = 01 02 03 04 05 06 0A FF, the screens at the
start, after walking and after inhaling are pixel-identical to the control
and WRAM and BW-RAM are byte-identical (target/harness/byte0.png). The patched
ROMs do differ at that byte (checked). Byte 0 is not read in room 1.
The same in each sub-game's first room (second part): byte 0 set to 00 and to
0A gives the same screen and the same BW-RAM; WRAM is identical too except
three bytes at 7E:1FE4-1FE7 in the cave's room 72 (stack residue, we think:
the screen is identical and nothing was seen to change). Not tested: byte 0
in other rooms than the first of each, or changed in every room at once.
"""

from emu import contact_sheet
from session import enter, patch, work_dir

VALUES = ["01", "02", "03", "04", "05", "06", "0a", "ff"]


def play(rom):
    e = enter(rom)
    frames = [e.shot()]
    e.run(120, ["Right"])
    frames.append(e.shot())
    e.run(60, ["Y"])
    e.run(120)
    frames.append(e.shot())
    return frames, e.wram(), e.bwram()


def diff(a, b):
    return sum(x != y for x, y in zip(a, b))


base = play(patch(1, "clear-objects"))
again = play(patch(1, "clear-objects"))
print(f"control vs control: wram {diff(base[1], again[1])} bwram {diff(base[2], again[2])}")
sheet = list(base[0])
for value in VALUES:
    frames, wram, bwram = play(patch(1, "clear-objects", "raw", "0", value))
    same = all(a.tobytes() == b.tobytes() for a, b in zip(frames, base[0]))
    print(f"byte0={value}: screens identical={same} wram diff {diff(base[1], wram)} bwram diff {diff(base[2], bwram)}")
    sheet += frames
out = work_dir() / "byte0.png"
contact_sheet(sheet, out, columns=3)
print("wrote", out)

# The same in the sub-games whose rooms carry a non-zero byte 0: Dyna Blade's
# room 28 (01), Gourmet Race's 68 (02), the cave's 72 (03), Revenge of Meta
# Knight's 154 (04), each set to 00 and to 0A, from the title screen on.
from session import enter_subgame  # noqa: E402

for name, room, original in (("dyna_blade", 28, 1), ("gourmet_race", 68, 2),
                             ("great_cave", 72, 3), ("revenge_of_meta_knight", 154, 4)):
    control = enter_subgame(name)
    c_shot, c_wram, c_bwram = control.shot().tobytes(), control.wram(), control.bwram()
    for value in ("00", "0a"):
        e = enter_subgame(name, patch(room, "raw", "0", value))
        print(f"{name:24} room {room:3} byte0 {original:02x} -> {value}: screen identical="
              f"{e.shot().tobytes() == c_shot} wram diff {diff(c_wram, e.wram())} bwram diff {diff(c_bwram, e.bwram())}")
