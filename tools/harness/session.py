"""Getting into the game quickly, with an original or a patched ROM.

Room 1 (Green Greens, the first playable room) is the test bench: Kirby
starts on flat ground at the left edge with the camera at the origin, so a
tile or an object placed in the first screen is visible at once.

    from session import patch, enter
    rom = patch(1, "clear-objects", "object", "0x01", "0", "110", "100")
    e = enter(rom)                 # in room 1, about 2 seconds of real time
    e.shot("out.png")

`enter` resumes from a savestate taken on the Spring Breeze stage card, before
the room is read from the ROM. That makes a run take about two seconds instead
of ten, and is why a state made with the original ROM works for a patched one.
The state is cached in `target/harness/`; delete it if the core changes.
"""

import os
import subprocess
from pathlib import Path

from emu import Emu, find_rom

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


def patch(room, *edits):
    """Builds a patched ROM with kss-core's own save path and returns its bytes.

    `edits` are the arguments of `cargo run -p kss-core --example patch_room`
    after the room number, e.g. `"tile", "3", "6", "3"`. Using the real save
    pipeline means every experiment also tests it.
    """
    out = work_dir() / "patched.sfc"
    command = ["cargo", "run", "-q", "-p", "kss-core", "--example", "patch_room", "--", str(out), str(room), *edits]
    result = subprocess.run(command, cwd=REPO, capture_output=True, text=True, env=_env())
    if result.returncode != 0:
        raise RuntimeError("patch_room failed:\n" + result.stderr)
    return out.read_bytes()


def _env():
    env = dict(os.environ)
    env.setdefault("KSS_ROM", str(find_rom()))
    return env
