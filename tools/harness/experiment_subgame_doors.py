"""Does an added door lead to a level-relative room in Dyna Blade?

    python tools/harness/experiment_subgame_doors.py

  question    In Spring Breeze a door's destination counts from the first
              room of the door's level (experiment_doors.py). Is that so in
              Dyna Blade (level 4 = rooms 27-32; the game starts in room 28)?
  hypothesis  Door to "3" leads to room 27 + 3 = 30. If the number indexed
              the room table it would lead to room 3, a Spring Breeze room.
  test        Room 28 with doors to "3" (record, star and door tile) on the
              cells around where Kirby starts, against the unchanged ROM.
              Kirby presses Up.
  evidence    Which room's whole tile map is at 7F:0000 afterwards, and the
              screenshots, target/harness/subgame-doors.png (after Up: control,
              door to 3, then the special numbers below).

Result: the door to 3 enters room 30, not room 3: level-relative in Dyna
Blade too. Destinations 0x4000 + n lead to the same room as n (0x4000 to room
27, 0x4007 to 34, 0x4008 to 35, all level 4's own rooms): bit 14 is a flag
and the rest the level-relative room. What the flag changes was not found.
Two tries before this one failed: a door needs its door tile (attribute 0x0F,
tile 0x4b here) as well as the record and the star, and the first guess of
Kirby's cell was a row too low.
"""

from emu import contact_sheet
from session import enter_subgame, loaded_room, patch, work_dir

ROOM = 28
DEST = 3
# A door needs its tile as well as its record and star (tried in Spring Breeze:
# the same door without the tile, room 1's 0x7e, does nothing). Room 28's own
# door, at cell (8, 20), is tile 0x4b (attribute 0x0F, like Spring Breeze's 0x7e). The camera starts at the top of the
# room and Kirby stands on the pink column at about x 112-128, feet at y 93:
# cell (4, 3) or (5, 3). Cells around that get a door, and Kirby's fires.
DOOR_TILE = "0x4b"
CELLS = [(x, y) for x in (4, 5) for y in (2, 3)]


def door_edits(dest):
    edits = []
    for cx, cy in CELLS:
        edits += ["tile", str(cx), str(cy), DOOR_TILE,
                  "object", "0x72", "0", str(cx * 24 + 12), str(cy * 24 + 12),
                  "door", str(dest), str(cx * 24), str(cy * 24), "0x24", "0x24"]
    return edits


def after_up(data):
    e = enter_subgame("dyna_blade", data)
    e.tap("Up", hold=8, wait=150)
    return loaded_room(e), e.shot()


frames = []
for label, data in (("control", None), ("door to 3", patch(ROOM, *door_edits(3)))):
    room, frame = after_up(data)
    frames.append(frame)
    print(f"{label:10} after Up: room {room} (None: an edited room, or not a room)")

# The special destinations of the original ROM, 0x4007 (room 167) and 0x4008
# (room 59), next to object 0x79: where does a door with such a number lead?
print("special destinations, door in room 28:")
for dest in (0x4000, 0x4001, 0x4007, 0x4008):
    room, frame = after_up(patch(ROOM, *door_edits(dest)))
    frames.append(frame)
    print(f"  door to {dest:#06x}: room {room}")

out = work_dir() / "subgame-doors.png"
contact_sheet(frames, out, columns=3)
print("wrote", out)
