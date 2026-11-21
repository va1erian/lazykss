"""Getting into the game quickly, with an original or a patched ROM.

Room 1 (Green Greens, the first playable room) is the test bench: Kirby
starts on flat ground at the left edge with the camera at the origin, so a
tile or an object placed in the first screen is visible at once.

    from session import patch, enter
    rom = patch(1, "clear-objects", "object", "0x01", "0", "110", "100")
    e = enter(rom)                 # in room 1, about 2 seconds of real time
    e.shot("out.png")

The other sub-games are reached with `enter_subgame("dyna_blade")` (also
"gourmet_race", "great_cave", "revenge_of_meta_knight"), which plays the
cork-board menu to the sub-game's first room: see SUBGAMES below.

`enter` resumes from a savestate taken on the Spring Breeze stage card, before
the room is read from the ROM. That makes a run take about two seconds instead
of ten, and is why a state made with the original ROM works for a patched one.
The state is cached in `target/harness/`; delete it if the core changes.
"""

import os
import subprocess
from pathlib import Path

from emu import Emu, find_rom
from kss import Rom

REPO = Path(__file__).resolve().parents[2]
WORK = REPO / "target" / "harness"
PRE_STATE = WORK / "before-green-greens.state"

# From power-on to the Spring Breeze stage card. Found by pressing a button,
# saving a screenshot, and looking: see docs/emulator-harness.md, "Navigating".
BOOT_FRAMES = 700
TO_STAGE_CARD = [
    "Start",  # skip the intro
    "Start",  # title screen -> file select
    "A",      # pick file 1
    "A",      # pick Spring Breeze on the cork board
    "A",      # confirm the game's description
    "A",      # "new game"
    "Right",  # "Is this your first time playing?" -> move to No
    "A",      # confirm No
    "Right",  # "Shall we explain copy?" -> move to No
]
# Confirm No, then the screens up to the stage card (longer waits).
THROUGH_CARD = ["A", "A", "A", "A"]
# From the stage card into the room.
INTO_ROOM = ["A", "A", "A"]


def work_dir():
    WORK.mkdir(parents=True, exist_ok=True)
    return WORK


def pre_state():
    """The cached savestate on the stage card, made on first use."""
    if not PRE_STATE.is_file():
        e = Emu()
        e.run(BOOT_FRAMES)
        for button in TO_STAGE_CARD:
            e.tap(button)
        for button in THROUGH_CARD:
            e.tap(button, wait=200)
        work_dir()
        PRE_STATE.write_bytes(e.state())
    return PRE_STATE.read_bytes()


def enter(rom=None, settle=20):
    """An emulator standing in Green Greens room 1 of `rom` (path or bytes).

    `settle` extra frames are run after Kirby gets control. Each call starts
    the game again, so calling it in a loop over many ROMs is fine.
    """
    state = pre_state()
    e = Emu(rom)
    e.run(5)
    e.load_state(state)
    for button in INTO_ROOM:
        e.tap(button, wait=200)
    for _ in range(2):
        e.tap("A", wait=150)
    e.run(settle)
    return e


def patch(room, *edits, base=None):
    """Builds a patched ROM with kss-core's own save path and returns its bytes.

    `edits` are the arguments of `cargo run -p kss-core --example patch_room`
    after the room number, e.g. `"tile", "3", "6", "3"`. Using the real save
    pipeline means every experiment also tests it. `base` is the path of a ROM
    to edit instead of the original (e.g. one with a room pointer swapped).
    """
    out = work_dir() / "patched.sfc"
    command = ["cargo", "run", "-q", "-p", "kss-core", "--example", "patch_room", "--", str(out), str(room), *edits]
    result = subprocess.run(command, cwd=REPO, capture_output=True, text=True, env=_env(base))
    if result.returncode != 0:
        raise RuntimeError("patch_room failed:\n" + result.stderr)
    return out.read_bytes()


def _env(base=None):
    env = dict(os.environ)
    if base:
        env["KSS_ROM"] = str(base)
    else:
        env.setdefault("KSS_ROM", str(find_rom()))
    return env


# ---- the other sub-games ----
#
# The cork-board menu (file 1, fresh save) is reached with Start, Start, A. On
# a fresh file the cursor skips Milky Way Wishes and Revenge of Meta Knight:
# they are locked. Byte 0x1A85 of the cartridge RAM holds how much is
# unlocked (1 on a fresh file, 2: Revenge of Meta Knight, 3: and Milky Way
# Wishes; found by writing each byte of the save area and watching whether
# the cursor reached the panel). 4 and 5 open nothing more that was seen
# (The Arena was not found); 6 plays a cut-scene and 8 and up garbage.
UNLOCK = 0x1A85

# name: how the cursor gets to the panel from Spring Breeze (the start), the
# value written to UNLOCK (0: leave it), the taps from the title screen
# ("new game") to the first room, and that room ("title": another entry whose
# title screen this one starts from). A tap waits 200 frames. The questions
# ("first time playing?", "explain copy?") are answered with Right then A; the
# game plays a guide or loading tips either way, and the counts of A were
# found by pressing until the first room's tile map appeared at 7F:0000
# (compared whole). Taps are timing-sensitive: keep the 200 frames.
SUBGAMES = {
    "dyna_blade": dict(cursor=["Down"], unlock=0, first_room=28,
                       after_title=["A", "Right", "A"] + ["A"] * 6),
    "gourmet_race": dict(cursor=["Right"], unlock=0, first_room=68,
                         after_title=["A", "A", "Right"] + ["A"] * 6),
    # The Gourmet Race menu opens on Grand Prix; Down once is course 1, then
    # courses 2 and 3 (rooms 68, 69, 70: levels 11, 12, 13).
    "gourmet_race_course2": dict(title="gourmet_race", first_room=69,
                                 after_title=["Down", "Down", "A", "A", "Right"] + ["A"] * 8),
    "gourmet_race_course3": dict(title="gourmet_race", first_room=70,
                                 after_title=["Down", "Down", "Down", "A", "A", "Right"] + ["A"] * 8),
    "great_cave": dict(cursor=["Down", "Right"], unlock=0, first_room=72,
                       after_title=["A", "Right", "A", "A", "A", "A", "A"]),
    # Meta Knight's crew speak before Kirby flies in: B, X and Y turn the
    # dialogue's pages, A and Start do not.
    "revenge_of_meta_knight": dict(cursor=["Down", "Right", "Right"], unlock=2, first_room=154,
                                   after_title=["A"] + ["B"] * 7),
}
BOARD_STATE = WORK / "cork-board.state"
BOARD_FRAMES = 200


def board_state():
    """The cached savestate on the cork board of a fresh file, made on first use."""
    if not BOARD_STATE.is_file():
        e = Emu()
        e.run(BOOT_FRAMES)
        for button in ["Start", "Start", "A"]:
            e.tap(button, wait=120)
        e.run(BOARD_FRAMES)
        work_dir()
        BOARD_STATE.write_bytes(e.state())
    return BOARD_STATE.read_bytes()


def _title_state(name):
    """The cached savestate on a sub-game's title screen, made on first use."""
    name = SUBGAMES[name].get("title", name)  # courses 2 and 3 share course 1's title
    path = WORK / f"title-{name}.state"
    if not path.is_file():
        spec = SUBGAMES[name]
        e = Emu()
        e.run(5)
        e.load_state(board_state())
        e.run(BOARD_FRAMES)  # the cursor ignores buttons for a moment after a load
        if spec["unlock"]:
            e.poke_bwram(UNLOCK, spec["unlock"])
        for button in [*spec["cursor"], "A", "A"]:
            e.tap(button, wait=200)
        path.write_bytes(e.state())
    return path.read_bytes()


def enter_subgame(name, rom=None, settle=20, record=False):
    """An emulator standing in the first room of sub-game `name` of `rom`.

    Like `enter`, it resumes from a savestate taken before the room is read
    from the ROM (the sub-game's title screen), so a patched ROM works. The
    room is `SUBGAMES[name]["first_room"]`; check the first screenshot, since
    the sub-game may open with dialogue or a cut-scene. With `record`, all
    sound from the title screen on is left in `e.audio` (see `Emu.record_audio`).
    """
    state = _title_state(name)  # before the Emu: making one restarts the core
    e = Emu(rom)
    e.run(5)
    e.load_state(state)
    if record:
        e.audio = bytearray()
    for button in SUBGAMES[name]["after_title"]:
        e.tap(button, wait=200)
    e.run(settle)
    return e


_ROOM_TILES = {}


def loaded_room(e, among=None):
    """Which room of the original ROM is in play: the one whose whole tile map
    is at 7F:0000 now, or None (menus, cut-scenes, a room that was edited).

    `among` limits the search to those room numbers. Rooms with identical
    tile maps cannot be told apart: the first is returned.
    """
    if not _ROOM_TILES:
        rom = Rom()
        for index in range(rom.room_count):
            try:
                _ROOM_TILES[index] = rom.room(index).tiles
            except (IndexError, ValueError):
                _ROOM_TILES[index] = []
    wram = e.wram()
    for index in (among if among is not None else sorted(_ROOM_TILES)):
        tiles = _ROOM_TILES[index]
        if len(tiles) >= 64 and tiles == [wram[0x10000 + i * 2] | wram[0x10001 + i * 2] << 8
                                          for i in range(len(tiles))]:
            return index
    return None
