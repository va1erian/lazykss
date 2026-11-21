"""Does an edited room load and play in each sub-game?

    python tools/harness/experiment_subgames.py

  question    The save pipeline was booted only in Spring Breeze. Does a room
              of another sub-game, saved through `kss-core`, load?
  test        For each sub-game's first room: the unchanged ROM (control), a
              small edit that fits the original place (some tiles), and the
              same edit with `noise`, which makes the room too big so that it
              is moved to free space and its pointer changed. Each is played
              in with `enter_subgame` and Kirby walks right for two seconds.
  evidence    (1) the whole tile map the game has at 7F:0000 equals the one
              kss.py (an independent reader) parses from the patched ROM;
              (2) whether the room pointer moved; (3) a contact sheet,
              target/harness/subgames-edit.png: per sub-game, control, small
              edit, noise, before and after walking.
"""

from emu import contact_sheet
from kss import Rom
from session import SUBGAMES, enter_subgame, patch, work_dir

# Tiles that make a visible difference near where Kirby starts. The tile
# numbers are ones the room itself uses, so the tileset has them.
EDITS = {
    "dyna_blade": ["tile", "0", "5", "0x6d", "tile", "1", "5", "0x6d", "tile", "2", "5", "0x6d"],
    "gourmet_race": ["tile", "6", "4", "0xc3", "tile", "7", "4", "0xc3", "tile", "8", "4", "0xc3"],
    "great_cave": ["tile", "5", "3", "0x6f", "tile", "6", "3", "0x6f", "tile", "7", "3", "0x6f"],
    # Kirby crosses the Halberd's deck: a band of tiles across the whole room,
    # so that some of it is on screen wherever the camera is.
    "revenge_of_meta_knight": [a for y in (17, 18) for x in range(0, 133)
                               for a in ("tile", str(x), str(y), "0xcd")],
}


def loaded_map_matches(e, data, room):
    """Whether the room at 7F:0000 is the one the patched ROM `data` holds."""
    reader = Rom()
    reader.data = data
    tiles = reader.room(room).tiles
    wram = e.wram()
    loaded = [wram[0x10000 + i * 2] | wram[0x10001 + i * 2] << 8 for i in range(len(tiles))]
    return loaded == tiles


def played(name, data, room):
    e = enter_subgame(name, data)
    frames = [e.shot()]
    match = loaded_map_matches(e, data, room)
    e.run(120, ["Right"])
    frames.append(e.shot())
    return frames, match


original = Rom()
sheet = []
for name in EDITS:
    room = SUBGAMES[name]["first_room"]
    cases = {
        "control": original.data,
        "edit": patch(room, *EDITS[name]),
        "noise": patch(room, *EDITS[name], "noise"),
    }
    for label, data in cases.items():
        frames, match = played(name, data, room)
        moved = Rom()
        moved.data = data
        print(f"{name:24} room {room:3} {label:8} map equals ROM: {match}   "
              f"pointer {'moved' if moved.room_offset(room) != original.room_offset(room) else 'same'}", flush=True)
        sheet += frames

out = work_dir() / "subgames-edit.png"
contact_sheet(sheet, out, columns=6)
print("wrote", out, "- rows: sub-games; per case: start, after walking right; cases: control, edit, noise")
