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

Levels 0-3 are Spring Breeze: Green Greens (rooms 0-5), Float Islands
(6-14), Bubbly Clouds (15-23), Mt. Dedede (24-26) **[rom]**. The first room
of each stage is its title card.

Level 15 is The Great Cave Offensive: one level of 81 rooms (72-152), the
only one holding treasure chests (§6a) **[rom]**. A level's room range ends
where the next level's begins.

## 3. Room block

At the address its room pointer gives **[rom]**:

```
+0    17  misc header: +2 song, +6 background scroll rate, +12 entrance style [community]
+17    5  FF
+22   24  graphics header (§5); bytes 20-23 are four object graphics slots
+46    1  a fifth object graphics slot (FF when unused)
+47    7  99 01 01 01 01 01 99 in every Spring Breeze room
+54    4  two words, unknown (non-zero in the first room of a stage)
+58       six sections; each starts with a 16-bit pointer relative to itself
          that leads to the next one. An empty section is 04 00 FF FF.
          Section 0 is the door list.
then   2  EF 3D in every room
       2  width, in cells
       2  height, in cells
       …  HAL-compressed map blob (§4)
```

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
- `7FFF` is a stage's goal door; its door star has variant 16 **[rom]**.
  Other values from `4000` up occur (`4008`, `4007`, next to object `79`);
  their meaning is not established.
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
packages. It holds addresses and layout only. 71 of the 87 `(id, variant)`
pairs used in Spring Breeze are covered; the bosses, the stage intro
objects, the door star (invisible) and a few objects that did not appear in
the capture are drawn as labelled boxes.

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
`kss_core::compression::compress` does an optimal parse and is never larger
than the original for the Spring Breeze rooms **[rom]**. The game's own
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
were booted and played in an emulator.

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
Greens, and reads WRAM, VRAM, OAM, the palette and screenshots.
[docs/emulator-harness.md](docs/emulator-harness.md) explains how to use it
and how each `[emu]` statement here was obtained. `cargo run -p kss-core
--example patch_room` produces the patched ROMs it runs, and `--example
render_room` writes room renders for comparison.

## 10. Sources

Room header, door, object and song notes: Super Famicom Development Wiki,
"Kirby Super Star" (Proto K, darklink). Compression format: Devin Acker's
exhal/inhal. Everything in §4-§6 and §8: this project.
