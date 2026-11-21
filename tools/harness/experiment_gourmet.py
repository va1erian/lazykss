"""Gourmet Race: what are objects 0x65, 0xC0-0xD2, and does the variant matter?

    python tools/harness/experiment_gourmet.py

  question    Course 1 (room 68) holds object 0x65 once, 0xC0 twice (variants
              0 and 16), 0xC1 (variant 16) in groups, and 0xC2-0xD2 with
              variants 0..39 that count up along the course. What do they
              look like, and does the variant change the object?
  test        Room 68 with its objects cleared, then (a) every id 0xC0-0xD2
              with variant 0 in a grid; (b) food 0xC2 with variants 0, 1, 5,
              20, 39; (c) food 0xD2 and 0xC0/0xC1/0x65 with several variants.
              Played in with `enter_subgame("gourmet_race")`.
  evidence    target/harness/gourmet.png: one frame per grid. Compare sprites
              across ids and across variants of one id.

Result: 0xC0 variant 0 is the numbered start sign ("1"), 0xC1 the red arrow
signs (variants turn the arrow), 0xC2-0xCF and 0xD0, 0xD1 are different foods
(one id, one food: the variant of a food does not change its look) and 0xD2
is a Maxim Tomato. 0x65 is King Dedede's start. 0xC0 variant 16 reached ends
the course with "CHECK POINT" and moves on to the next course; variant 32
shows "GOAL!" and the results screen. Without a goal object the screen stays
black, and without the start sign nothing moves (second part, gourmet-goal.png).
"""

from emu import contact_sheet
from session import enter_subgame, patch, work_dir

Y_TOP, Y_LOW = 60, 108


def grid(objects):
    """`object` edits for (id, variant) pairs on two rows, 24 pixels apart."""
    edits = []
    for n, (ident, variant) in enumerate(objects):
        row, col = divmod(n, 9)
        edits += ["object", hex(ident), str(variant), str(60 + col * 24), str(Y_TOP if row == 0 else Y_LOW)]
    return edits


GRIDS = {
    "ids 0xC0-0xD2, variant 0": [(i, 0) for i in range(0xC0, 0xD3)],
    "0xC2 variants 0 1 5 20 39": [(0xC2, v) for v in (0, 1, 5, 20, 39)],
    "0xD2 variants 0 1 5 20 39": [(0xD2, v) for v in (0, 1, 5, 20, 39)],
    "0xC0 var 0 16 32, 0xC1 var 0 16 32, 0x65 var 0 1 2": [(0xC0, 0), (0xC0, 16), (0xC0, 32),
                                                           (0xC1, 0), (0xC1, 16), (0xC1, 32),
                                                           (0x65, 0), (0x65, 1), (0x65, 2)],
}

frames = []
for label, objects in GRIDS.items():
    e = enter_subgame("gourmet_race", patch(68, "clear-objects", *grid(objects)))
    frames.append(e.shot())
    print("shot:", label)
out = work_dir() / "gourmet.png"
contact_sheet(frames, out, columns=2)
print("wrote", out)

# What ends the race? Every course has an 0xC0 of variant 0 near the start
# (140, 132 in course 1; Kirby cannot move without it, see below), Dedede
# (0x65) and an 0xC0 of variant 16 (32 in course 3) at the far end of the
# room. Rooms with only the start objects, with a variant-16 object put
# 260 pixels ahead of Kirby, and with a variant-32 one, run for 12 seconds
# holding Right; the timer and the screen show whether the race ended.
START = ["object", "0x65", "0", "52", "132", "object", "0xC0", "0", "140", "132"]
CASES = {
    "start objects only": START,
    "+ 0xC0 variant 16 at x=400": START + ["object", "0xC0", "16", "400", "124"],
    "+ 0xC0 variant 32 at x=400": START + ["object", "0xC0", "32", "400", "124"],
    "no start sign (0xC0 variant 0 removed)": ["object", "0x65", "0", "52", "132"],
}
print("running:")
frames = []
for label, edits in CASES.items():
    e = enter_subgame("gourmet_race", patch(68, "clear-objects", *edits))
    for _ in range(4):
        e.run(180, ["Right"])
        frames.append(e.shot())
    print(" ", label)
sheet = work_dir() / "gourmet-goal.png"
contact_sheet(frames, sheet, columns=4)
print("wrote", sheet, "- one case per row, 3 seconds between frames, holding Right")
