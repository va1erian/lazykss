"""Doors: do added ones work, and what does a destination count from?

    python tools/harness/experiment_doors.py

  question 1  The editor can now add doors. That grows section 0 of the room
              block and moves sections 1-5 (section 3 lists the breakable
              cells). Does the game still read the moved sections, and does
              it use the added door?
  question 2  Does a door's destination index the room pointer table, or
              count from the first room of the door's level?
  test        Room 1 with two star blocks in Kirby's way and their records,
              and a door on the cell Kirby starts in (1, 6), stored as
              leading to "2". ROMs:
                control   blocks and records only
                appended  the same plus the door, appended to the list
                cleared   the door list emptied first, then the door
                level     "appended", with Green Greens' entry of the level
                          table moved 6 rooms on, so the stage starts at
                          Float Islands' rooms, and that room copied over
                          room-table entry 7 (the stage's second room)
  evidence    For each ROM, one run presses Up on the start cell and reads
              which room's tile map is at 7F:0000; another inhales twice and
              shows whether the blocks broke.

Result (target/harness/doors.png; per row: start, after Up, after the
inhales): control stays in room 1; appended and cleared enter room 2 and
their blocks break; level enters room 8, not room 2. Destinations count from
the level's first room.

The first attempt put the door on (1, 5), the cell above Kirby, and nothing
happened: a door is entered from the cell its position names.
"""

from emu import contact_sheet
from kss import MASTER, Rom
from session import enter, patch, work_dir

rom = Rom()
ROOMS = {n: rom.room(n).tiles[:16] for n in (1, 2, 8)}

# Star blocks in Kirby's way (see experiment_blocks.py) and their records.
BLOCKS = ["tile", "3", "6", "3", "tile", "4", "6", "3",
          "block", "3", "6", "0", "0", "block", "4", "6", "0", "0"]
# The cell Kirby stands in at the start, given the tile of the room's own
# door (cell (16, 5)) and the invisible door star at the cell's centre.
CELL = (1, 6)
DOOR_TILE = ["tile", str(CELL[0]), str(CELL[1]), "0x7e",
             "object", "0x72", "0", str(CELL[0] * 24 + 12), str(CELL[1] * 24 + 12)]
DOOR = ["door", "2", str(CELL[0] * 24), str(CELL[1] * 24), "0x9c", "0x6c"]


def into_float_islands(data):
    """Green Greens starts at room 6, and room 1's block is its second room."""
    data = bytearray(data)
    levels = MASTER + rom.u16(MASTER + 12)
    first = data[levels] | data[levels + 1] << 8
    data[levels:levels + 2] = (first + 6 * 4).to_bytes(2, "little")
    table = rom.room_table
    data[table + 7 * 4:table + 8 * 4] = data[table + 1 * 4:table + 2 * 4]
    return bytes(data)


appended = patch(1, "clear-objects", *BLOCKS, *DOOR_TILE, *DOOR)
CASES = {
    "control": patch(1, "clear-objects", *BLOCKS),
    "appended": appended,
    "cleared": patch(1, "clear-objects", *BLOCKS, *DOOR_TILE, "clear-doors", *DOOR),
    "level": into_float_islands(appended),
}


def current_room(e):
    w = e.wram()
    head = [w[0x10000 + i * 2] | w[0x10001 + i * 2] << 8 for i in range(16)]
    return next((n for n, tiles in ROOMS.items() if tiles == head), None)


frames = []
for name, data in CASES.items():
    e = enter(data)
    frames.append(e.shot())
    e.tap("Up", hold=8, wait=150)
    frames.append(e.shot())
    room = current_room(e)
    e = enter(data)
    for _ in range(2):
        e.run(50, ["Right"])
        e.run(100, ["Y"])
        e.run(40)
    frames.append(e.shot())
    print(f"{name:9} Up on the door cell -> room {room}")

out = work_dir() / "doors.png"
contact_sheet(frames, out, columns=3)
print("wrote", out)
