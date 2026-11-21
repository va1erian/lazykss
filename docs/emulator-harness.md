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
| `tools/harness/session.py` | `enter(rom)` puts you in Green Greens room 1 in about two seconds; `enter_subgame(name, rom)` does the same for the other sub-games; `loaded_room(e)` says which room is in play; `patch(room, ...)` builds an edited ROM through `kss-core`'s real save code. |
| `tools/harness/selfcheck.py` | Verifies the three files above against the ROM. |
| `tools/harness/probe.py` | Traces VRAM contents back to ROM resources; finds where a compressed stream starts. |
| `tools/harness/experiment_blocks.py` | A complete worked experiment. **Copy it to start a new one.** |
| `tools/harness/experiment_doors.py` | Added doors, and what a door's destination counts from. |
| `tools/harness/experiment_treasures.py` | Opens a chest of every treasure number and saves the windows. |
| `tools/harness/experiment_byte0.py` | Byte 0 of the room block, changed in room 1 and in each sub-game's first room: nothing differs. |
| `tools/harness/experiment_levels.py` | One contact sheet per level 4-44: the level's first room, with Green Greens' level-table entry moved. |
| `tools/harness/experiment_subgames.py` | A tile edit and a relocating (`noise`) edit of each sub-game's first room, played in. |
| `tools/harness/experiment_subgame_doors.py` | An added door in Dyna Blade is level-relative; the `0x40nn` destinations. |
| `tools/harness/experiment_music_bit.py` | Bit 7 of the music byte, by recording the sound. |
| `tools/harness/experiment_gourmet.py` | Gourmet Race objects: signs, foods, check point and goal. |
| `tools/harness/experiment_romk.py` | Revenge of Meta Knight's dialogue object 0xBC, all 25 variants. |
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
| `e.record_audio(frames)` | the sound of those frames | 16-bit stereo, 32 kHz; the core is deterministic, so equal bytes are equal sound |
| `e.poke_bwram(offset, value)` | writes a byte of cartridge RAM | the unlock byte, see "The other sub-games" |

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
`door DEST X Y DEST_X DEST_Y` (appends a door), `clear-doors`,
`clear-objects`, `slots HEX8`, `raw OFFSET HEX`, `noise`. Numbers may be
`0x`-prefixed.

`patch` compiles the working tree. An experiment that runs for minutes
while you edit `kss-core` fails halfway when the tree stops compiling: build
`patch_room` once into its own target directory
(`cargo build -p kss-core --example patch_room --target-dir target/frozen`)
and call that binary for long runs.

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

To play **another level**, move Green Greens' entry of the level table (the
word at directory entry 6, `0x3F0000 + word(0x3F000C)`) on by `4 * n`
bytes: the stage then starts `n` rooms later in the room table, and its
"room 1" is room `n + 1`. Doors count their destination from there.
`experiment_doors.py` does this to tell a level-relative destination from
an absolute one.

To **enter a door**, Kirby must stand in the cell the door's position names
and press Up (`e.tap("Up")`). At the start of room 1 that is cell (1, 6),
door position (24, 144).

## The other sub-games

Everything above was found in Spring Breeze. The cork-board menu offers more;
`session.py` plays it for you:

```python
from session import enter_subgame, loaded_room, patch
e = enter_subgame("dyna_blade")                  # in its first room, about 3 s
e = enter_subgame("great_cave", patch(72, "noise", "tile", "5", "3", "0x6f"))
loaded_room(e)                                   # which room's whole tile map is at 7F:0000
```

| Name | First room | Notes |
|---|---|---|
| `dyna_blade` | 28 | The game starts in the *second* room of level 4 (rooms 27-32), a tall 11x23 room, Kirby on a pink column. Room 27 (176x8, palm trees) is reached by door `0x4000`. |
| `gourmet_race`, `gourmet_race_course2`, `_course3` | 68, 69, 70 | Single-course races (rooms 68-70 are levels 11-13). |
| `great_cave` | 72 | |
| `revenge_of_meta_knight` | 154 | Needs the unlock below. Meta Knight's crew speak first; B, X and Y turn the pages, A and Start do not. |

The routes were found as described under "Getting somewhere else": a tap, a
screenshot, a look. What is on the board of a fresh file **[emu]**: Spring
Breeze, Gourmet Race, Dyna Blade, The Great Cave Offensive and Megaton Punch
(and a sound test and a stereo/mono switch at its edge). The cursor
**skips Milky Way Wishes and Revenge of Meta Knight**: they are locked.
`enter_subgame` caches savestates at the board and at each title screen
(`target/harness/cork-board.state`, `title-*.state`), like `pre_state`. A tap
needs its 200 frames and the cursor ignores buttons for a moment after a
state is loaded; both are built into the helpers. Making an `Emu` restarts
the core, so fetch the cached state before creating the `Emu` that will use it.

**The unlock** is byte `0x1A85` of the cartridge RAM (`e.bwram()`): 1 on a
fresh file. Written 2, Revenge of Meta Knight opens and was played
(`SUBGAMES[...]["unlock"]`); written 3, Milky Way Wishes opens too. It was
found by writing 0xFF over each 128, then 16, then 1 byte of the saved area
and watching whether the cursor reached the panel. 4 and 5 open nothing more
that was seen; 6 plays a cut-scene; 8 and up gave garbage. **Not found:** The
Arena (no panel appeared for any value tried), and a way to enter a planet of
Milky Way Wishes: with 3 the title, the intro and the star map load and the
star can be flown with the D-pad, but A, B, X, Y, Start and Up did nothing
over the nearest planet in the time spent. `enter_subgame` has no
`milky_way` for that reason.

### Levels 4-44 of the level table

Found by making Green Greens' entry of the level table point at the level
(`experiment_levels.py`, one contact sheet per level) and by the real routes.
"Real" means the sub-game's own mode was played from the menu; the rest is the
first room of the level shown in Spring Breeze's mode (its stage-intro sprites
come out as garbage there and its HUD is Spring Breeze's).

| Level | Rooms | What | Basis |
|---|---|---|---|
| 4 | 27-32 | Dyna Blade, Peanut Plains | real: the game starts in room 28 |
| 5 | 33-42 | Dyna Blade, Mallow Castle | doors `0x4007`/`0x4008` from room 28 land in a castle garden (34) and a castle hall (35) |
| 6 | 43-51 | Dyna Blade, Cocoa Cave | brown cave; by order |
| 7 | 52-62 | Dyna Blade, Candy Mountain | snowy peaks at night; by order |
| 8 | 63-65 | Dyna Blade's Nest (last stage) | mountain slope; by order, least sure |
| 9, 10 | 66, 67 | single castle interiors with item rooms | not identified; probably parts of Mallow Castle |
| 11, 12, 13 | 68, 69, 70 | Gourmet Race courses 1, 2, 3 | real |
| 14 | 71 | 11x22 sky shaft, no objects | not identified |
| 15 | 72-152 | The Great Cave Offensive | real |
| 16 | 153 | 11x80 vertical room, clouds at the top | not identified (the cave's exit?) |
| 17 | 154-156 | Revenge of Meta Knight, Prologue (Warp Star ride to the Halberd, sunset) | real |
| 18-25 | 157-196 | Revenge of Meta Knight, the chapters in order: 18 shore, 19 forest, 20 deck with propeller engines, 21 main cannon, 22 right wing, 23 engine room, 24 interior (helper HUD), 25 reactor | order, backgrounds and dialogue; chapter names and numbers not checked |
| 26-34 | 197-261 | Milky Way Wishes: 26 Floria (four 121x8 rooms), 27 lava planet, 28 cloud planet, 29 night cliffs, 30 water planet, 31 machine planet, 32 star field, 33 purple jungle, 34 Nova (object 0x60, Marx's room 261) | Spring Breeze-mode first rooms only; **the planet names are guesses**, not played |
| 35 | 262-283 | The Arena (22 rooms, Marx in 283) | by room count and Marx's song; not played |
| 36 | 284 | one 11x8 room, Whispy Woods' trunk on grass | not identified |
| 37-41 | 285-297 | the guide's curtain rooms: 37 holds the "explain copy" guide (room 288 was in play in Dyna Blade's, **real**), 39 the Gourmet Race guide, 40 the cave guide (matching backdrops); 38 and 41 other guides' curtain rooms | partly real |
| 42, 43, 44 | 298, 299, 300 | a 176x8 sea panorama, a forest room with an enemy bar (11x8), a pink/teal 11x32 room | not identified |
| 45-59 | 301-318 | 2x2 dummy rooms | |

Door destinations count from the first room of the level the door is in and
can run past that level's end (a door in room 28 to `0x4007` enters room 34,
which is level 5) **[emu]**.

### What was seen in each sub-game

- **Byte 0 of the room block** does nothing visible. Room 1 with byte 0 set
  to 01, 02, 03, 04, 05, 06, 0A and FF, and each sub-game's first room set
  to 00 and 0A: same screens, same BW-RAM, same WRAM (the cave: three WRAM
  bytes differ at 7E:1FE4, screens identical) **[emu]**
  (`experiment_byte0.py`). Which sub-game a room belongs to is decided by the
  level table and the menu. Not tested: changing it in every room at once,
  or in rooms other than the first.
- **Edited rooms load and play** in Dyna Blade (room 28), Gourmet Race (68),
  The Great Cave Offensive (72) and Revenge of Meta Knight (154): a small
  in-place tile edit shows, and an edit with `noise` (the pointer moved to
  another bank) loads: the tile map at 7F:0000 equals the one an independent
  reader parses from the patched ROM, and the noise is on screen **[emu]**
  (`experiment_subgames.py`). Kirby walked for two seconds without a problem;
  the rooms were not played through.
- **An added door in Dyna Blade** (room 28, destination 3) enters room 30:
  level-relative, as in Spring Breeze **[emu]** (`experiment_subgame_doors.py`).
  A door needs its door tile (metatile attribute 0x0F: 0x4B in Dyna's
  tileset, 0x7E in Green Greens') as well as the record and the star.
  Destinations `0x4000 | n` lead to the same room as `n`; what the flag
  changes was not found.
- **Music byte, bit 7**: with it set the room does not start its song, and the
  song that was playing is stopped, not kept. Room 1 with 0x8F instead of
  0x0F: the stage card's song stops after the room change, a short quiet
  tail, then silence for the rest of the recording, where 0x0F and 0x10 play
  on. In Revenge of Meta Knight's room 154 (0xB3) the first four seconds (the
  dialogue) are silent; with 0x33 the song starts at once; the music starts
  anyway later (a script starts it) **[emu]**, by recording the sound
  (`experiment_music_bit.py`). Not found: whether the low bits still matter
  when bit 7 is set (the four bit-7 values gave different, tiny tails). Song
  0x0A is a three-second jingle that stops by itself.
- **Gourmet Race**: 0x65 is King Dedede's start place; 0xC0 variant 0 the
  numbered start sign (without it nothing moves); 0xC0 variant 16 reached
  shows CHECK POINT and moves on to the next course, variant 32 shows GOAL!
  and the results screen (courses 1 and 2 end with 16, course 3 with 32); a
  course without a goal object stays black. 0xC1 are arrow signs (the
  variant turns the arrow), 0xC2-0xD1 one food per id (the variant does not
  change the look; in the original it counts 0..39 along the course), 0xD2
  is a Maxim Tomato **[emu]** (`experiment_gourmet.py`). Not tested: what the
  food variant (the counter) is used for, 0xC1 variant 16 in groups near the
  end.
- **Revenge of Meta Knight**: object 0xBC's variant is the number of the
  dialogue (25 lines, 0-24, each with the speaker's portrait; see
  `romk-dialogue.png`) and it starts when Kirby is near the object **[emu]**
  (`experiment_romk.py`). 0xBD was not looked at. The HUD shows a countdown
  ("TIME", 2900 at the start, counting down through the dialogue and play)
  **[emu]**; what happens at zero, and the escape timer of the last chapter,
  were not tested.
- **Not tested at all**: Dyna Blade's map switches (0x7F, 0xAB), what the
  `0x40nn` flag does, Milky Way Wishes' copy essences (0x94/0x95) and whether
  their variant picks the ability, The Arena's room order and its rest room.

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
9. **A door one cell off.** A door put on the cell above Kirby's head
   (where the room's own door tile sits, one row up) never triggered, which
   looked like "added doors do not work". Kirby stands in row 6 at the start
   of room 1; the door must be on his cell.
10. **Guessing an address.** Every address in `sprite_table.rs` came from a
   byte-exact match against the running game. Extend it with `sprites.py`,
   not by hand.

## What the harness cannot tell you

- **Why** the game does something: there is no trace of the code. You learn
  what data it reads and what happens when the data changes.
- Anything about **timing or audio**.
- Whether a ROM works on **real hardware** or in another emulator. A saved
  ROM was also played in ares by hand; that is the cross-check.
- Milky Way Wishes and The Arena have not been entered (see "The other
  sub-games"). The other sub-games were entered and their first rooms played
  for seconds, not played through; The Great Cave Offensive's treasure chests
  and doors were studied in room 1 and through its data, not by playing the
  cave.

## Reporting what you found

When you add to the documentation:

- Mark it `[emu]` only if the running game was seen to agree, and say how in
  one line ("placed in room 1, inhaled, block disappeared").
- Mark it `[rom]` if it was checked against the ROM bytes and a test covers
  it.
- Otherwise say it is a guess, or leave it out.
- List what you did not test next to what you did.
- Keep the experiment script if it would be needed to re-check the claim.
