"""A worked experiment: what makes a star block breakable?

    python tools/harness/experiment_blocks.py

Copy this file to start a new experiment. It shows the whole method:

  question    Star blocks placed in room 1 cannot be broken. Why?
  hypothesis  A block needs a record in section 3 of the room (rooms that
              have working blocks all have one; room 1 has none).
  test        Two ROMs that differ in exactly one thing: the records.
  evidence    Screenshots before and after Kirby inhales, side by side.

Result (target/harness/blocks.png): without records the blocks stay; with
records Kirby eats them. The top row is the control.

Two things that went wrong the first time, kept here as warnings:

- The blocks were first placed one row too high. Kirby walked under them, and
  that looked like "blocks are not solid". Always check in the first
  screenshot that the thing under test is where the test needs it.
- A test that shows nothing happening proves little without a control. Run
  the unchanged case next to the changed one, with the same inputs.
"""

from emu import contact_sheet
from session import enter, patch, work_dir

# Metatile 3 is the star block of the Green Greens tileset. Row 6 is the
# ground Kirby walks on, so blocks there are in his way.
BLOCKS = ["tile", "3", "6", "3", "tile", "4", "6", "3"]
RECORDS = ["block", "3", "6", "0", "0", "block", "4", "6", "0", "0"]

frames = []
for edits in (BLOCKS, BLOCKS + RECORDS):
    e = enter(patch(1, "clear-objects", *edits))
    e.run(50, ["Right"])       # walk up to the blocks
    frames.append(e.shot())
    e.run(100, ["Y"])          # inhale
    e.run(40)
    frames.append(e.shot())
    e.run(60, ["Right"])       # step forward and inhale again
    e.run(100, ["Y"])
    e.run(40)
    frames.append(e.shot())

out = work_dir() / "blocks.png"
contact_sheet(frames, out, columns=3)
print("wrote", out, "- top row: no records, bottom row: with records")
