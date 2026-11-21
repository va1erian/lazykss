"""Revenge of Meta Knight: what does object 0xBC's variant do?

    python tools/harness/experiment_romk.py

  question    Every room of Revenge of Meta Knight has an 0xBC and/or 0xBD
              (room 154: 0xBC variant 0 at (284, 476) near Kirby's start and
              0xBD variant 15 at (2181, 429); 155: 0xBC 1; 156: 0xBC 2 and 0xBD
              8). The opening shows Meta Knight's crew talking. Is the
              variant the number of the dialogue?
  test        Room 154 with all objects cleared and one 0xBC of each variant
              put where the original is; played from the title screen with
              one tap of A, then 5 seconds, and the dialogue box is on screen.
              (The Warp Star ride and the enemies are gone, so only the
              dialogue shows.)
  evidence    target/harness/romk-dialogue.png: the dialogue box of each
              variant, cropped to the box. One variant per frame.

Result: yes. The variant picks the line and the speaker's portrait (0 "K-Kirby's
here! He's heading towards us!", 1 "Kirby's in the rocket valve!", 5 "He fell
into the forest.", 8 "...close to the twin cannon.", 17 "Major damage to the
right wing!", 23 "We are losing power. Engine No.3!"). In the original, each
variant is used in one place only, apart from 0xBD variant 11. The variant is
global, not per level. The dialogue starts when Kirby is near the object.
"""

from PIL import Image, ImageDraw

from emu import Emu
from session import _title_state, patch, work_dir

VARIANTS = list(range(25))


def dialogue_of(variant):
    data = patch(154, "clear-objects", "object", "0xBC", str(variant), "284", "476")
    state = _title_state("revenge_of_meta_knight")
    e = Emu(data)
    e.run(5)
    e.load_state(state)
    e.tap("A", wait=200)
    e.run(300)
    return e.shot().crop((0, 168, 256, 224))


sheet = Image.new("RGB", (512, 56 * 13))
for n, variant in enumerate(VARIANTS):
    box = dialogue_of(variant)
    x, y = n // 13 * 256, n % 13 * 56
    sheet.paste(box, (x, y))
    ImageDraw.Draw(sheet).text((x + 2, y + 44), f"variant {variant}", fill=(255, 255, 0))
out = work_dir() / "romk-dialogue.png"
sheet.save(out)
print("variants", VARIANTS[0], "-", VARIANTS[-1], "->", out)
