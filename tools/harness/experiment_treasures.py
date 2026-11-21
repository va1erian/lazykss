"""What is each treasure of The Great Cave Offensive called, and worth?

    python tools/harness/experiment_treasures.py

  question    A treasure chest is object B8 and its variant is the treasure
              number (the cave's 58 chests use 58 different variants). What
              does each number hold?
  test        For each variant 0-63: room 1 with every object removed, the
              object graphics slots of room 74 (a room with a chest), and one
              chest at (110, 156), a few steps right of Kirby. Walk into it,
              press Up, wait for the window.
  evidence    The window the game opens, cropped: "<name> Obtained! Value:
              <n>G". Four sheets of 16 in target/harness/treasures0-3.png,
              each crop numbered with its variant.

Result: variants 0-59 name the 60 treasures (kss-core/src/treasure.rs);
60 and up open an empty window. The values also sit in the ROM at 0xF4B1A
(5-byte records, a 24-bit value first); treasure.rs has a test that checks
the table against it. Room 1 is outside the cave: whether a chest there also
counts towards the cave's collection was not looked at.
"""

from PIL import Image, ImageDraw

from kss import Rom
from session import enter, patch, work_dir

slots = Rom().room(74).object_slots
crops = []
for variant in range(64):
    e = enter(patch(1, "clear-objects", "slots", slots, "object", "0xb8", str(variant), "110", "156"))
    e.run(50, ["Right"])
    e.run(30)
    e.run(10, ["Up"])
    e.run(360)
    crop = e.shot().crop((30, 36, 226, 100)).resize((392, 128))
    draw = ImageDraw.Draw(crop)
    draw.rectangle((0, 0, 30, 14), fill=(0, 0, 0))
    draw.text((2, 2), str(variant), fill=(255, 255, 0))
    crops.append(crop)
    print("variant", variant)

for part in range(4):
    sheet = Image.new("RGB", (392 * 4, 128 * 4))
    for k, crop in enumerate(crops[part * 16:(part + 1) * 16]):
        sheet.paste(crop, ((k % 4) * 392, (k // 4) * 128))
    out = work_dir() / f"treasures{part}.png"
    sheet.save(out)
    print("wrote", out)
