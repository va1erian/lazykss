# Kirby Super Star (SNES, USA) — level data reference

What `lazykss` knows about the ROM. Every statement marked **[emu]** was
checked against the running game (VRAM, palette and OAM captured from a
bsnes core); **[rom]** means checked against the ROM bytes and exercised by
the `kss-core` tests; **[community]** comes from the Super Famicom
Development Wiki and has not been verified here.

An earlier version of this document described 16x16 metatiles made of four
tilemap words, a "collision table" at graphics-header byte 2 and a palette
"script" bytecode. All three were wrong; see §5 for what is there instead.

All offsets are PC file offsets unless written `bank:offset`.

## 1. The ROM

| Property | Value |
|---|---|
| File | 4,194,304 bytes, no copier header (a 512-byte header is stripped on load) |
| SHA-1 | `cca1e67fa230bf4f60154f5ea55d06568229dc5d` |
| Internal title | `KIRBY SUPER DELUXE` at `0x7FC0` |
| Mapping | SA-1; banks `C0-FF` map the image linearly |
| Checksum | `0xC6AA` at `0x7FDE`, complement at `0x7FDC` |

`pc = snes - 0xC00000` for every long pointer in the data tables **[rom]**.
The checksum is the 16-bit sum of all bytes; `Rom::fix_checksum` recomputes
it after every save.

The game logic runs on the SA-1 with its work RAM in BW-RAM (`6000-7FFF`),
not in WRAM. WRAM still holds a CGRAM mirror at `7E:0500` (512 bytes) and
the decoded tile map of the current room at `7F:0000` (16-bit ids,
`width` words per row) **[emu]**.

## 2. Master directory (bank `FF`)

`0x3F0000` is a list of 16-bit offsets into bank `FF`.

| Entry | Offset | Table |
|---|---|---|
| 0 | `009A` | Room pointers: 319 entries of 4 bytes (long pointer + `00`) |
| 1 | `0596` | Resource pointers: 258 entries of 3 bytes, entry 0 null |
| 2 | `089C` | End of the resource table (another pointer table follows) |
| 6 | `0022` | Level table: 60 words, each the address of a level's first room pointer |

A level is a run of consecutive rooms; a level's room range ends where the
next level's begins. The level table does not say which sub-game a level
belongs to: §2a does.

## 2a. Sub-games and their levels

Levels 0-35 belong to the seven sub-games played through rooms; levels 36-59
are cut-scene, guide and dummy rooms. The room ranges are **[rom]**. Byte 0 of
every room block of a sub-game holds the same value (column "Byte 0"; the
`kss-core` tests check every room), but the game ignores it: room 1 with
byte 0 set to 01-06, 0A or FF, and the first room of four sub-games set to 00
and 0A, played with identical screens and identical BW-RAM **[emu]**
(`tools/harness/experiment_byte0.py`). The sub-game a room plays in comes
from the menu.

| Sub-game | Byte 0 | Levels | Rooms |
|---|---|---|---|
| Spring Breeze | 00 | 0-3 | 0-26 |
| Dyna Blade | 01 | 4-10 | 27-67 |
| Gourmet Race | 02 | 11-14 | 68-71 |
| The Great Cave Offensive | 03 | 15-16 | 72-153 |
| Revenge of Meta Knight | 04 | 17-25 | 154-196 |
| Milky Way Wishes | 05 | 26-34 | 197-261 |
| The Arena | 06 | 35 | 262-283 |
| (guide theatre, demo, dummies) | 07-0A | 36-59 | 284-318 |

The levels, and how each was identified:

| Level | Rooms | Name | Basis |
|---|---|---|---|
| 0-3 | 0-26 | Green Greens (0-5), Float Islands (6-14), Bubbly Clouds (15-23), Mt. Dedede (24-26) | **[emu]** played; the first room of each stage is its title card |
| 4 | 27-32 | Peanut Plains | **[emu]** played from the menu. The game starts in room 28, not 27: room 27 (176x8) is entered by door `4000` |
| 5 | 33-42 | Mallow Castle | **[emu]** doors `4007`/`4008` from room 28 land in its garden and hall |
| 6-8 | 43-65 | Cocoa Cave, Candy Mountain, Dyna Blade's Nest | by order and backgrounds; level 8 holds Dyna Blade (object `55`) |
| 9, 10 | 66, 67 | "Secret Area 1/2" | guess: single castle interiors full of ability items |
| 11-13 | 68, 69, 70 | Gourmet Race courses 1-3 | **[emu]** played from the menu |
| 14 | 71 | an 11x22 sky shaft with no objects | not identified |
| 15 | 72-152 | The Great Cave Offensive | **[emu]** played from the menu; the only level with treasure chests (§6a) |
| 16 | 153 | an 11x80 vertical room, clouds at the top | not identified |
| 17 | 154-156 | Revenge of Meta Knight's prologue (the Warp Star chase) | **[emu]** played from the menu |
| 18-25 | 157-196 | named after their rooms: the shore, the forest, the deck, the main cannon, the right wing, the engine room, the interior, the reactor | order from backgrounds, dialogue and the HUD; the game's chapter numbers and titles were not checked |
| 26-33 | 197-258 | Floria (four 121x8 rooms, one per season), Hotbeat, Skyhigh, Cavios, Aqualiss, Mekkai, Halfmoon, ??? | from the rooms' looks (lava, clouds, red cave, water, machines, moons, jungle); no planet was entered. The ROM order is not the order of the star map |
| 34 | 259-261 | Galactic Nova (Nova's heart, object `60`; Marx in 261) | from the objects |
| 35 | 262-283 | The Arena (Marx's room is 283) | from the room count and songs; not played |

The guide's curtained theatre rooms are levels 37-41: room 288 was the
loaded room during Dyna Blade's copy guide **[emu]**.

What was played in each sub-game, and how to get there with the harness, is
in [docs/emulator-harness.md](docs/emulator-harness.md). Milky Way Wishes
and The Arena were not entered.

## 3. Room block

At the address its room pointer gives **[rom]**:

```
+0    17  misc header: +0 sub-game tag (§2a, unused by the game), +2 music (below),
          +6 background scroll rate, +12 entrance style [community]
+17    5  FF
+22   24  graphics header (§5); bytes 20-23 are four object graphics slots
+46    1  a fifth object graphics slot (FF when unused)
+47    7  99 01 01 01 01 01 99 in 266 of 319 rooms (every Spring Breeze room);
          the others hold 02s and 03s between the 99s. Not understood
+54    4  two words, unknown (non-zero in the first room of a stage)
+58       six sections; each starts with a 16-bit pointer relative to itself
          that leads to the next one. An empty section is 04 00 FF FF.
          Section 0 is the door list.
then   2  EF 3D in every room
       2  width, in cells
       2  height, in cells
       …  HAL-compressed map blob (§4)
```

The **music** byte's low 7 bits are the song. **Bit 7 means the room starts
no song** and the one playing stops **[emu]** (recorded audio,
`tools/harness/experiment_music_bit.py`: Green Greens room 1 with `8F`
instead of `0F` falls silent after the stage card; Revenge of Meta Knight's
room 154, `B3`, is silent through its dialogue and plays at once with `33`).
The sub-games' dialogue and boss rooms use it (`B3`, `83`, `B5`, `93`, `A2`,
`BA`, `AD`); a script starts their music later.

Doors are 10-byte records, `FF FF` terminated:

```
dest_room(2)  x(2)  y(2)  dest_x(2)  dest_y(2)
```

Positions are pixels.

- **`dest_room` counts from the first room of the door's level**, not from
  the start of the room pointer table **[emu]**. Float Islands' room 7 leads
  to "2", which is room 8; The Great Cave Offensive's room 72 leads to "2",
  room 74. Shown by `tools/harness/experiment_doors.py`: the same door,
  stored as 2, entered room 2 from Green Greens and room 8 once Green Greens'
  level-table entry was moved to start at room 6. Across the ROM, 266 doors
  have a door back from their target when read this way, 2 when read as
  table indices **[rom]**. In Spring Breeze's first stage the two readings
  agree, which is why this went unnoticed.
  This holds in every sub-game: a door added to Dyna Blade's room 28 with
  destination 3 entered room 30 **[emu]**, and all 426 doors of levels
  0-35 read this way lead into their own level **[rom]**. A destination may
  run past the end of its level, and the game follows it.
- `7FFF` is a stage's goal door; its door star has variant 16 **[rom]**.
- **`4000 | n` leads to room `n`** as `n` does **[emu]**: from Dyna Blade's
  room 28, `4000` entered room 27, `4007` room 34 and `4008` room 35. The
  original rooms use it twice (room 59 → `4008`, room 167 → `4007`, both
  next to object `79`). What the flag changes was not found; the editor
  keeps it when the destination room is edited.
- **A door works only on a door tile**: the trigger cell must hold a metatile
  whose attribute is `0F` **[emu]**. A record and a door
  star on any other cell did nothing, in Spring Breeze and in Dyna Blade. 372
  of the 407 original doors with a trigger sit on one; most of the others
  sit under a breakable block (attributes `70`-`7F`) or on attribute `8F`
  (Aqualiss's rooms 232-236, untested; the editor accepts it as a door tile).
- `x`, `y` is the top-left corner of the trigger cell: the game's door stars
  (object `72`, invisible) sit at `x + 12, y + 12`, and Kirby enters a door
  by pressing Up while in that cell **[emu]**: a door put on the cell above
  Kirby did nothing, on his own cell it worked.
- `x = y = FF00` marks a door with no trigger area. A **warp star** (object
  `75`) uses one: its variant is the index of its record in the door list
  (room 1's warp star has variant 1 and door 1 has no trigger; room 134's
  has variant 8, its ninth door) **[rom]**. The game does not validate
  `dest_room`.

## 4. Map blob

```
width*height   tile id low bytes, row-major
width*height   tile id high bytes
6 bytes each   objects: variant, id, x(2), y(2)
FF FF
```

**A cell is 24x24 pixels (3x3 characters), not 16x16** **[emu]**. Green
Greens room 1 is 100x8 cells, of which the first 45 columns are used; the
door star object at `(0x18C, 0x84)` sits on the door drawn at cell (16, 5).
Object and door coordinates are pixels from the room's top-left corner.

Tile id 0 is an ordinary, fully transparent metatile.

## 5. Graphics

The graphics header indexes the resource table. Resources are stored three
ways: compressed, raw, and raw-header-plus-compressed.

| Byte | Resource | Storage |
|---|---|---|
| 0 | flag (`00` in boss rooms) | |
| 1 | `AA` | |
| 2 | Foreground characters, loaded at character 0 | compressed 4bpp, 704 characters |
| 3, 4 | Foreground palette (same index twice) | raw |
| 5-6 | Metatile table (16-bit index: `00 01` = entry 256) | compressed |
| 7 | Background characters, loaded at character `0x280` | compressed 4bpp |
| 8, 9 | Background palette | raw |
| 10 | Background tilemap | raw `width, height` words, then compressed words |
| 11-18 | More character sets (animated tiles), unused here | compressed |
| 19 | Extra palette, applied first | raw |
| 20-23 | Object graphics slots, with a fifth at room offset +46 — a different id space (§6) | |

A **raw palette** is `count, first_colour` then `count` BGR555 words, written
to CGRAM from `first_colour` **[emu]**. Green Greens: `50 00` (rows 0-4) for
the foreground and `30 50` (rows 5-7) for the background.

Both layers share one character space: the background set is loaded at
`0x280` and the foreground set over its start, so tilemap words of either
layer index the same 0-`0x3FF` range **[emu]**.

A **metatile table** decompresses to `19 * n` bytes (`n` = 720 for Green
Greens): nine planes of `n` tilemap words — one plane per position of the
3x3 block, row-major — then one plane of `n` attribute bytes (collision
class; `00` empty, `50` solid ground, …; semantics not mapped). Words are SNES
tilemap entries `vhopppcc cccccccc` **[emu]**.

The background is a tilemap of whole 8x8 characters (64x32 for Green Greens)
that the game scrolls at its own rate; the editor pins it to the room origin.

Known gaps: animated characters (a few foreground characters are overwritten
at run time from the sets in bytes 11-18), and the background palette of
King Dedede's arena renders with wrong colours.

## 6. Object sprites

Object graphics are not in the resource table. A pointer table at `0x101726`
(69 long pointers) lists compressed **packages** **[emu]**:

```
+0    1      slot class bits: 01 02 04 08 10 → OBJ palette rows B C D E F and
             character bases 0A0 060 0C0 0F8 130
+1    5x32   palettes: 0F, CGRAM index, 15 BGR555 words
             (0 normal, 1-3 alternates, 4 the hit flash)
+161  nx32   4bpp characters
```

Shared sheets are plain compressed character streams: common effects at
`0x212E30` (loaded at character `0x160`), items at `0x1BEE40` and stars at
`0x1CC01B` (both copied into VRAM on demand).

Frames are lists of 5-byte records, mostly in bank `0D` (bank `05` for
Kirby and items):

```
flags  dx  dy  character  attributes     flags: bit 0 = 16x16, bit 1 = last piece
```

`dx`/`dy` are signed offsets from the object position; `character` and
`attributes` are OAM bytes with absolute character numbers.

Which package and frame an object id uses is decided by object code, not by
a table. `kss-core/src/sprite_table.rs` therefore records it per object: it
was generated by placing each Spring Breeze object in Green Greens room 1,
running the patched ROM, and tracing the characters in OAM back to the
packages. It holds addresses and layout only. 480 of the 738 `(id, variant)`
pairs used anywhere in the game are covered (rooms 0-318). Per sub-game,
covered of used: Spring Breeze 75 of 87, Dyna Blade 156 of 178, Gourmet Race
5 of 182, The Great Cave Offensive 251 of 262, Revenge of Meta Knight 174
of 219, Milky Way Wishes 233 of 253, The Arena 30 of 48 (a pair is counted
in every sub-game that uses it). An object whose graphics are not in the
room 1 test bench was captured with its source room's graphics header, or
with room 1 made a copy of that room **[emu]**. Not covered, so drawn as
labelled boxes: most bosses (`50`-`5E`, `60`, `61`), the stage intro objects
(`A0`-`A6`), invisible markers (`72`, `7E`, `7F`, `19`, `05`) and the
Gourmet Race objects (`65`, `C0`-`D2`), whose characters are not in any
object graphics package or resource stream.

## 6a. Treasures (The Great Cave Offensive)

A treasure chest is object `B8`; its **variant is the treasure number**,
0-59 **[emu]**. The number alone decides what the chest holds: a chest of
each number placed in Green Greens room 1 and opened showed that treasure's
name and value (`tools/harness/experiment_treasures.py`; the table is
`kss-core/src/treasure.rs`). Numbers from 60 open an empty window.

The values are also in the ROM, at `0xF4B1A`: 60 records of 5 bytes, a
24-bit value then a word holding a permutation of 0-59 whose use is not
established **[rom]**. They add up to 9,999,990 G.

The cave's rooms hold 58 chests, one for every treasure except 0 (Gold
Medal) and 16 (Saucepan) **[rom]**. How those two are obtained was not looked
at. Neither was what happens when two chests share a number, or whether a
chest outside the cave counts towards the collection.

## 6b. Objects of the other sub-games

Objects whose variant carries a number, from the emulator **[emu]** unless
marked:

- **Gourmet Race** (`tools/harness/experiment_gourmet.py`):
  - `65`: King Dedede's start place.
  - `C0` is a sign: variant 0 is the start (without it the race never
    starts), 16 a checkpoint ("CHECK POINT", then the next course), 32 the
    goal ("GOAL!", then the results). Courses 1 and 2 end with 16, course 3
    with 32; a course with neither stays on a black screen.
  - `C1`: red arrow signs; the variant turns the arrow.
  - `C2`-`D1`: one food each. The variant does not change the look; in the
    original courses it counts up along the course (0-72), and what it is
    for was not found.
  - `D2`: a Maxim Tomato.
- **Revenge of Meta Knight** (`tools/harness/experiment_romk.py`):
  - `BC` is a dialogue: its variant is the line (0-24, global across the
    sub-game, each with its speaker's portrait), shown when Kirby comes
    near. Line 0 is the crew spotting Kirby.
  - The HUD's TIME countdown (2900 in the prologue) is not in the room
    data that was looked at.
  - `BD` (in every level) was not identified.
- **Ability items and copy essences** **[rom]**, not tested: `90`, `91` and
  Milky Way Wishes' `95` use variants that are multiples of `10`
  (`00`-`C0`), which suggests the ability in the high nibble.
- **Items**: Maxim Tomatoes (`82`), 1-Ups (`80`) and foods (`83`-`8D`)
  carry a different low number in their variant from item to item (the
  cave's tomatoes run from `40` to `5F`) **[rom]**. What the number does was
  not tested.
- Not studied: Dyna Blade's switches (`7F`, `AB`), the Arena's rest room,
  Milky Way Wishes' copy essence pedestals.

## 7. HAL compression

A stream of chunks ended by `FF`. A chunk starts with `CCCLLLLL` (command,
length - 1) or the long form `111CCCLL LLLLLLLL` (10-bit length - 1).

| Command | Meaning |
|---|---|
| 0 | literal bytes |
| 1 | one byte repeated |
| 2 | two bytes repeated (length counts pairs) |
| 3 | incrementing byte run |
| 4 | copy from an absolute big-endian offset in the output |
| 5 | same, each byte bit-reversed |
| 6 | same, reading backwards |

Offsets are absolute, so a stream cannot be decompressed from its middle.
`kss_core::compression::compress` does a near-optimal parse (it tries the
longest run of each command, and 128 earlier matches). It is never larger
than the original for 318 of the 319 rooms; Skyhigh's room 213 re-encodes
one byte longer (516 bytes for 515), so saving it moves it to free space
**[rom]**. The game's own
decompressor has accepted every stream it produced so far (the rooms booted
in §8), which is not proof for every command mix.

## 8. Saving

`Room::save` re-encodes the block and **[emu]**:

- writes it over the original when it is no longer than before, padding the
  rest with `FF`;
- otherwise writes it into the `FF` padding at the end of a bank (the
  smallest that fits, skipping 16 guard bytes), repoints the room, and
  fills the old block with `FF`.

Only bank-tail padding is treated as free (about 32 KiB in total): runs of
`FF` inside a bank can be data. Rooms are self-contained and referenced only
through the room pointer table, so they can live in any bank. Both paths
were booted and played in an emulator: in Spring Breeze, and for the first
rooms of Dyna Blade (28), Gourmet Race (68), The Great Cave Offensive (72)
and Revenge of Meta Knight (154), where the tile map in WRAM matched the
saved room after an in-place edit and after a forced move to another bank
(`tools/harness/experiment_subgames.py`; a few seconds of play each, no
room played through). Milky Way Wishes and The Arena rooms were not booted.

### Breakable blocks

Section 3 of the room block lists the breakable cells, 6 bytes each, `FF FF`
terminated **[emu]**:

```
cell_index * 2 (2)   metatile after breaking (2)   third word (2)
```

A star or bomb block (metatile attribute `70`-`7F`) is solid from the tile
alone, but Kirby can only break it when its cell has a record here: the same
star block was inert without one and inhaled with one. The first word is the
byte offset of the cell in the 16-bit map. The third word is often the id of
the cap metatile above the block and often 0; breaking works with 0, and its
purpose is not established. The original rooms also list decoration cells
(attributes `27`, `28`, `A8`) next to blocks, and leave some attribute-`76`
cells unlisted.

`Room::sync_blocks` adds a record for every block placed since the room was
loaded and drops the record of a block that was painted over; the editor
calls it on save.

A room holds at most 30,000 cells: both tile planes and the object list
must fit one 64 KiB stream.

### Adding and removing doors

The six sections are found by following their relative pointers, so section
0 can change size and the rest move with it. `Room::set_doors` rewrites the
door list that way **[emu]**: room 1 with a door appended, and with its list
emptied and refilled, booted, the added door took Kirby to its destination,
and the star blocks listed in the moved section 3 still broke
(`tools/harness/experiment_doors.py`).

Warp stars name their door by index, so removing a door renumbers the warp
stars of the doors after it; the editor does that in the same undo step.

## 9. Tools used to establish this

`tools/harness/` is a headless emulator for scripts (Python `ctypes` over a
libretro bsnes core): it boots a ROM, replays the inputs that lead into Green
Greens or the first room of another sub-game, and reads WRAM, VRAM, OAM, the palette and screenshots.
[docs/emulator-harness.md](docs/emulator-harness.md) explains how to use it
and how each `[emu]` statement here was obtained. `cargo run -p kss-core
--example patch_room` produces the patched ROMs it runs, and `--example
render_room` writes room renders for comparison.

## 10. Sources

Room header, door, object and song notes: Super Famicom Development Wiki,
"Kirby Super Star" (Proto K, darklink). Compression format: Devin Acker's
exhal/inhal. Everything in §4-§6 and §8: this project.
