# The emulator harness

How to ask the running game a question, and how the formats in
[KSS_Level_Editor_Documentation.md](../KSS_Level_Editor_Documentation.md) were
found. Written for whoever works on this next, human or agent. Follow it
step by step; none of it needs intuition about the SNES.

## The one rule

**When the ROM bytes can be read two ways, do not argue from the bytes. Run
the game and look.**

The first version of this project spent days on static analysis and got the
tile format wrong three ways, each of them self-consistent: sizes divided
evenly, values stayed in range, and the output was garbage. One screenshot
and one VRAM dump settled all three in an hour. A statement about the game
is only marked `[emu]` in the documentation when the running game was seen
to agree with it.

## Setup

You need:

- Python 3 with `pillow` (and `numpy`, for `sprites.py` only).
- A bsnes-family **libretro core** (it must emulate the SA-1 chip). The
  harness was built on `bsnes_mercury_balanced_libretro.dll` from a RetroArch
  install. Set `KSS_LIBRETRO_CORE` to its path. Without it, `emu.py` falls
  back to the path it had on the machine it was written on (`DEFAULT_CORE`),
  which will not exist on yours.
- The ROM. Set `KSS_ROM`, or keep `Kirby Super Star (USA).sfc` in a parent
  directory. It is never committed.
- A working `cargo` (the harness builds patched ROMs with `kss-core`).

Then run the self-check. Do this first, every time you start on a new
machine:

```text
python tools/harness/selfcheck.py
```

Every line must say `ok`. Open `target/harness/selfcheck.png` and confirm it
shows Kirby on grass under a blue sky. If a line fails, its hint says where
to look. Do not go on with a failing self-check: everything below would be
measuring something else.

All output goes to `target/harness/`, which is not committed.

## The files

| File | What it is |
|---|---|
| `tools/harness/emu.py` | The emulator: boot a ROM, press buttons, run frames, read memory, take screenshots. No window. |
| `tools/harness/kss.py` | An independent Python reader for the ROM (decompressor, rooms, resources). Independent on purpose: it checks `kss-core` rather than repeating it. |
| `tools/harness/session.py` | `enter(rom)` puts you in Green Greens room 1 in about two seconds; `patch(room, ...)` builds an edited ROM through `kss-core`'s real save code. |
| `tools/harness/selfcheck.py` | Verifies the three files above against the ROM. |
| `tools/harness/probe.py` | Traces VRAM contents back to ROM resources; finds where a compressed stream starts. |
| `tools/harness/experiment_blocks.py` | A complete worked experiment. **Copy it to start a new one.** |
| `tools/harness/sprites.py` | Regenerates `kss-core/src/sprite_table.rs`. |

## What you can read

```python
from session import enter, patch
e = enter()                    # original ROM, standing in room 1
e.run(60, ["Right"])           # hold Right for 60 frames (one second)
e.shot("target/harness/a.png") # what is on screen
```

| Call | Gives | Notes |
|---|---|---|
| `e.wram()` | 128 KiB work RAM | index 0 = `7E:0000`, index `0x10000` = `7F:0000` |
| `e.vram()` | 64 KiB video RAM | 4bpp character `n` is the 32 bytes at `n * 32` |
| `e.bwram()` | 8 KiB cartridge RAM | the SA-1 runs the game logic here (`6000-7FFF`) |
| `e.palette()` | 256 colours, BGR555 | read from the game's mirror at `7E:0500` |
| `e.oam()` | sprites on screen | `(x, y, character, attributes, large)`, front first |
| `e.shot()` | the frame, 256x224 | a PIL image |
| `e.state()` | the core's savestate | everything, in the core's own layout |

Known places in memory while a room runs:

| Where | What |
|---|---|
| `7F:0000` | the room's tile map, 16-bit ids, `width` words per row |
| `7F:3000` | the tileset's metatile table (nine word planes, then attributes) |
| `7E:0500` | CGRAM mirror, 512 bytes |
| VRAM characters `000-27F` | foreground set; `280-3FF` background set |
| VRAM `0xC000` and up | sprite characters (OAM character `n` is VRAM character `0x600 + n`) |
| OAM characters `00-5F` | Kirby; everything else is objects and effects |

## Room 1, the test bench

Experiments are run in Green Greens room 1 because it is the first room and
Kirby starts in a known place. What you need to know to put something where
the test needs it:

- **Positions are pixels**, measured from the room's top-left corner. Object
  and door coordinates use them directly. A map **cell is 24x24 pixels**:
  cell `(cx, cy)` covers pixels `24*cx .. 24*cx+23` by `24*cy .. 24*cy+23`.
  `tile X Y` and `block X Y` take cells; `object ... X Y` takes pixels.
- The room is 100x8 cells. The ground Kirby walks on is **row 6**; row 7 is
  below it. Kirby starts at about pixel x 30, standing with his feet at about
  y 157. He is 16 pixels tall, so he occupies y 141-157: a block in row 5
  (y 120-143) is above his head and he walks under it.
- At the start the camera is at the room's left edge, so **screen x = room
  x**. Vertically the picture is shifted up by about 8 pixels (room y 120 is
  drawn at screen y 112). Check positions in a screenshot rather than
  trusting this to the pixel.
- The camera follows Kirby: once he is near the middle of the screen he
  stays there and the room scrolls instead. Kirby "stopping" at screen x
  120 while you hold Right is the camera moving, not a wall.
- Things placed in the first 256 pixels are visible without walking.

About objects:

- Object ids are listed in `kss-core/src/names.rs` and in the documentation,
  section 4. `0x01` is a Waddle Dee, `0x82` a Maxim Tomato.
- An enemy's graphics are only loaded if the room's **object graphics
  slots** include them; otherwise it shows as garbage. Items and stars use a
  shared sheet and need nothing. To place an enemy that room 1 does not
  normally have, copy the slots of a room that has it:
  `kss.Rom().room(N).object_slots` gives the hex for `patch(..., "slots", hex)`.
  `sprites.py` does exactly this.
- To tell whether an item was **picked up** or an enemy **defeated**, watch
  it leave OAM: count `[s for s in e.oam() if s[2] >= 0x60]` before and
  after, and confirm in the screenshot. The health bar is no evidence while
  Kirby is at full health.

There is **no debugger**: no breakpoints, no trace, no register view. You get
snapshots of memory and pictures. Every method below is built from "change
one thing, take a snapshot, compare".

## The method

Use this for every question. `experiment_blocks.py` is all six steps in forty
lines; read it before writing your own.

1. **Write the question as one sentence** that a picture or a memory compare
   can answer. "Why can't star blocks placed in room 1 be broken?"
2. **Write down what you expect to see** if your guess is right, and what you
   expect if it is wrong. If both guesses would produce the same picture, the
   experiment is useless: change it.
3. **Change exactly one thing.** Build two ROMs with `patch` that differ only
   in the thing under test.
4. **Always run a control**: the unchanged case, with the same button
   presses, in the same script. "Nothing happened" means nothing without it.
5. **Look at the result yourself.** Save a contact sheet
   (`emu.contact_sheet`) and open the PNG. Do not conclude from the script
   finishing without an error.
6. **Write down what was shown and what was not.** "Star blocks break with a
   record" was shown; "bomb blocks chain-react" was not tested, and the
   documentation says so.

### Recipe: where does something on screen come from?

The game's memory is the answer sheet. Whatever is in VRAM came out of the
ROM, so search the ROM for the exact bytes.

```text
python tools/harness/probe.py vram
```

prints, for each run of VRAM characters, which resource holds the same
32 bytes and at what position. That is how "header byte 2 is the foreground
character set, loaded at character 0" was established: no reasoning, just a
match. An exact 32-byte match is not a coincidence.

If nothing matches, the data is compressed somewhere the resource table does
not list. Search the **raw** ROM for one character: compressed streams store
unrepeated data as literal runs, so a character often appears verbatim inside
its stream. Then find where that stream starts:

```text
python tools/harness/probe.py stream 0x242BB2 0x800
```

Back-references in this compression are absolute, so a stream decoded from
the wrong start gives wrong bytes. The candidates are printed; the right one
decodes to a sensible length and starts with a sensible header. This is how
the object graphics packages were found.

### Recipe: what does this byte or record do?

Patch it, run both versions, compare pictures.

```python
rom_a = patch(1, "clear-objects")
rom_b = patch(1, "clear-objects", "raw", "33", "0a")   # one header byte changed
```

Each `patch` call runs `cargo run` (the first one compiles, later ones take
about a second) and each `enter` about two seconds.

`patch` takes the arguments of
`cargo run -p kss-core --example patch_room -- <out> <room> ...`:
`tile X Y ID`, `object ID VARIANT X Y`, `block X Y AFTER EXTRA`,
`clear-objects`, `slots HEX8`, `raw OFFSET HEX`, `noise`. Numbers may be
`0x`-prefixed.

To find which of several differences between two rooms matters, copy them
across one at a time (bisect). Compare rooms that work with rooms that do
not: the answer to the star-block question came from printing every room's
sections next to whether the room contains blocks.

### Recipe: does a saved ROM still work?

Any change to `Room::save`, the compressor, or the room layout must be booted.
`patch` goes through the real save code, so:

```python
e = enter(patch(1, "tile", "3", "6", "3", "noise"))   # "noise" forces relocation
e.run(200, ["Right"])
e.shot("target/harness/saved.png")
```

Check both paths: a small edit (saved in place) and one with `noise` (the
room no longer fits and is moved). Walk through the room, not just into it.

### Recipe: an object's sprite

```text
python tools/harness/sprites.py --sheet
```

places each object alone in room 1, reads what the game put in OAM, and
traces every 8x8 character and the palette back to ROM addresses. It rewrites
`kss-core/src/sprite_table.rs` and saves `target/harness/sprites.png`. Run on
an unchanged tree, it must reproduce the committed table exactly (`git diff`
shows nothing): that is the check that the harness still behaves as it did.

Objects it leaves out are listed with the reason. To cover them, the object
must be made to appear (bosses need their own room) and its characters must
be in a source the script knows (`SHARED_SHEETS`, the package table).

## Getting somewhere else in the game

`session.py` holds the button presses from power-on to room 1. They were
found the slow, reliable way: press one button, save a screenshot, look,
repeat. Do the same for a new route:

```python
from emu import Emu, contact_sheet
e = Emu(); e.run(700)
frames = []
for button in ["Start", "Start", "A", "A"]:
    e.tap(button)
    frames.append(e.shot())
contact_sheet(frames, "target/harness/route.png")
```

To study **another room**, do not play to it. Make room 1 *be* that room:
copy the 4-byte entry of the room pointer table (at `0x3F009A + 4 * room`)
over entry 1 in a copy of the ROM, and `enter` it. Kirby still starts at the
left edge near the ground, so check the first screenshot to see where he
landed.

## Mistakes already made, so you do not repeat them

1. **Trusting sizes that divide evenly.** The metatile table is 13,680 bytes:
   that is 1,710 records of 8 bytes *and* 720 records of 19. The wrong one was
   "verified" for weeks. Divisibility proves nothing; a matching picture does.
2. **Decompressing data that is not compressed.** Palettes are raw. A
   decompressor that never reports errors returned plausible garbage, which
   was then studied as a "bytecode". `kss.decompress` raises on a bad
   back-reference; treat that as information, not as a nuisance.
3. **A test object in the wrong place.** Star blocks placed one row too high
   let Kirby walk underneath, which looked like "blocks are not solid". In
   the first screenshot, check that the thing under test is where the test
   needs it. Cells are 24 pixels; Kirby stands in row 6 of room 1.
4. **No control.** See the method, step 4.
5. **Cutting test output short.** `cargo test | head` hid a failing test for
   two commits. Read every `test result:` line.
6. **Tests that skip silently.** Tests needing the ROM return early without
   it, so they pass anywhere, including CI. To prove they really ran, point
   `KSS_ROM` at a junk file: they must then fail.
7. **Assuming one backend speaks for all.** Headless snapshots use the
   software renderer. A crash that only happened on Direct2D passed every
   test. What was not run on the real thing is not verified: say so.
8. **Reading a savestate layout as if it were documented.** OAM is found at a
   fixed distance after VRAM in this core's savestate. Another core, or
   another version, will differ; `selfcheck.py` is what tells you.
9. **Guessing an address.** Every address in `sprite_table.rs` came from a
   byte-exact match against the running game. Extend it with `sprites.py`,
   not by hand.

## What the harness cannot tell you

- **Why** the game does something: there is no trace of the code. You learn
  what data it reads and what happens when the data changes.
- Anything about **timing or audio**.
- Whether a ROM works on **real hardware** or in another emulator. A saved
  ROM was also played in ares by hand; that is the cross-check.
- Rooms outside Spring Breeze have been loaded and rendered, not played.

## Reporting what you found

When you add to the documentation:

- Mark it `[emu]` only if the running game was seen to agree, and say how in
  one line ("placed in room 1, inhaled, block disappeared").
- Mark it `[rom]` if it was checked against the ROM bytes and a test covers
  it.
- Otherwise say it is a guess, or leave it out.
- List what you did not test next to what you did.
- Keep the experiment script if it would be needed to re-check the claim.
