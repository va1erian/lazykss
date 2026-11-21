"""Which stage is each level of the level table?

    python tools/harness/experiment_levels.py [first [last]]

  question    Levels 4-44 of the level table: which sub-game and stage?
  test        Green Greens' entry of the level table is made to point at the
              level under test (docs/emulator-harness.md, "To play another
              level"), and the Spring Breeze route is played into it. The
              sub-game mode stays Spring Breeze: this shows the level's own
              data (stage card, backdrop, tileset), not the sub-game's rules.
  evidence    Per level, the frames after each of the presses that lead from
              the stage card into the first room, as one contact sheet
              (target/harness/levels-N.png), five frames per level.
"""

import sys

from emu import Emu, contact_sheet
from kss import MASTER, Rom
from session import INTO_ROOM, loaded_room, pre_state, work_dir

rom = Rom()


def level_start_word(level):
    return rom.u16(MASTER + rom.u16(MASTER + 12) + 2 * level)


def level_first_room(level):
    return (level_start_word(level) - (rom.room_table - MASTER)) // 4


def starting_at(level):
    """The ROM with Green Greens' level-table entry copied from `level`."""
    data = bytearray(rom.data)
    entry = MASTER + rom.u16(MASTER + 12)
    data[entry:entry + 2] = level_start_word(level).to_bytes(2, "little")
    return bytes(data)


def frames_into(level):
    state = pre_state()
    e = Emu(starting_at(level))
    e.run(5)
    e.load_state(state)
    frames = [e.shot()]
    for button in [*INTO_ROOM, "A", "A"]:
        e.tap(button, wait=150)
        frames.append(e.shot())
    return frames, loaded_room(e)


if __name__ == "__main__":
    first = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    last = int(sys.argv[2]) if len(sys.argv) > 2 else 44
    for level in range(first, last + 1):
        out = work_dir() / f"levels-{level}.png"
        frames, room = frames_into(level)
        contact_sheet(frames, out, columns=6)
        print("level", level, "starts at room", level_first_room(level), "-> playing room", room, flush=True)
