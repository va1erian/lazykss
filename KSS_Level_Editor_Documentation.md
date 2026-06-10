# Building a Level Editor for Kirby Super Star (SNES)
## Complete Technical Documentation

Everything in this document marked **[verified]** was checked byte-for-byte against the exact ROM you provided during this analysis session. The rest comes from community documentation (Data Crystal, the Super Famicom Development Wiki, and Devin Acker's exhal project), with sources cited at the end.

---

## 1. Your ROM, identified

Your file is the No-Intro verified good dump of the US release, headerless, with a valid internal checksum. **[verified]**

| Property | Value |
|---|---|
| File size | 4,194,304 bytes (32 Mbit) — no copier header |
| MD5 | `cb76ea8ac989e71210c89102d91c6c57` |
| SHA-1 | `cca1e67fa230bf4f60154f5ea55d06568229dc5d` |
| Internal title | `KIRBY SUPER DELUXE` (header at file offset `0x7FB0`) |
| Map mode | `0x23` — **SA-1, LoROM-style** |
| Cartridge type | `0x35` — ROM + SA-1 + RAM + battery (8 KiB SRAM) |
| Internal checksum | `0xC6AA` (recomputed from the file: matches) |
| Region / publisher | USA / Nintendo (maker `01`), game code `AKFE` |

This matters for two reasons. First, your editor should verify it is opening this exact dump (hash check) before trusting any hardcoded offsets, and refuse or warn on the Japanese/European versions, whose data lives at different addresses. Second, the SA-1 chip has real consequences, covered next.

## 2. The SA-1 elephant in the room

Kirby Super Star is an SA-1 cartridge: a second 65C816 CPU running at 10.74 MHz with its own memory mapper (the "Super MMC"). For a level editor this changes surprisingly little about *data editing* — you are reading and writing a flat 4 MB file — but it changes three practical things.

The first is address translation. Community documents about this game mix three address styles freely: PC file offsets (e.g. `3020EF`), SNES addresses in the SA-1 HiROM-style region (`F0:20EF`), and bank-`FF` shorthand (`FF:0000`). (SNES literature usually prefixes these with a dollar sign — `$F0:20EF` — but this document omits it because many Markdown viewers misinterpret paired dollar signs as math notation and scramble the text.) With the Super MMC in its power-on default configuration (which is how all the static pointer tables in this game are meant to be read), banks `C0–FF` map the 4 MB ROM linearly, so the conversion is simply:

```
PC file offset  =  SNES address − 0xC00000        (for banks C0–FF)
SNES address    =  PC file offset + 0xC00000
```

So `FF:0000` = file offset `0x3F0000`, and `F0:20EF` = file offset `0x3020EF`. This conversion was confirmed directly: the room pointer table entry for Green Greens room 1 stores the long pointer `F0:20EF`, and the room's data is at file offset `0x3020EF`. **[verified]** The lower banks (`00–3F` / `80–BF`) are bankswitched in 1 MB blocks through SA-1 registers `2220–2223`, but you only need to care about that if you start reverse-engineering code; the level data tables all use `C0+` long pointers.

The second consequence is testing. Use an emulator with accurate SA-1 support and a real debugger — **Mesen 2** (best debugger: memory viewer, tilemap viewer, breakpoints on the SA-1 CPU) or **bsnes/ares** (accuracy reference). Avoid ZSNES-era tooling entirely; SA-1 support there is broken.

The third is the checksum. After any edit your editor must recompute the internal checksum, or accurate emulators and flash carts may complain. For this 4 MB power-of-two ROM the algorithm is trivial: sum all bytes mod `0x10000`, write the result little-endian at file offset `0x7FDE`, and write its complement (`checksum XOR 0xFFFF`) at `0x7FDC`.

## 3. The master pointer table — the spine of the whole game

The last 64 KiB of the ROM — bank `FF`, file offsets `0x3F0000–0x3FFFFF` — begins with a master directory. It is simply a list of 16-bit words. Each word is itself a pointer: an offset, counted from the start of bank `FF`, telling you where another table begins inside that same bank. In other words, the game keeps all of its important data tables in one bank, and this directory at the very start is the index that locates each of them. **[verified]**

The first words in your ROM are:

```
3F0000: 9A 00  96 05  9C 08  3F 01  02 01  43 00  22 00  2A 00 ...
```

The entries that matter most for a level editor:

| Master entry | Points to (offset in bank FF) | Contents |
|---|---|---|
| `FF:0000` | `009A` | **Room pointer table** |
| `FF:0002` | `0596` | **Graphics / palette pointer table** |
| `FF:000C` | `0022` | Level pointer table (groups rooms into levels, `0x3C` = 60 entries) |

### 3.1 The room pointer table (`FF:009A`, file `0x3F009A`)

Each entry is a **4-byte little-endian long pointer** into the `C0–FF` region (the 4th byte is always `00` padding). The table runs from `0x3F009A` to `0x3F0596`, giving **(0x596 − 0x9A) / 4 = 319 rooms**. **[verified]** Example entries from your ROM:

```
index 0: CE DA F0 00  →  F0:DACE  →  file 0x30DACE
index 1: EF 20 F0 00  →  F0:20EF  →  file 0x3020EF   ← Green Greens, room 1
index 2: C4 81 F0 00  →  F0:81C4  →  file 0x3081C4
```

Note that rooms are stored in a scrambled order relative to gameplay; the *level* table at `FF:0022` (60 word-sized entries, each an offset into bank `FF` marking where a level's run of room pointers begins) is what groups them into the eight sub-games' stages. Your editor will want a friendly room index built from both tables, plus the music byte trick described in §4.2 for identifying rooms by ear.

### 3.2 The graphics/palette pointer table (`FF:0596`, file `0x3F0596`)

Unlike the room table, this one uses **3-byte long pointers** (no padding byte), with a null `00 00 00` first entry. **[verified]** Each pointer targets a HAL-compressed blob (see §6) containing either 4bpp tile graphics or palette data. Example from your ROM: entry 4 is `00 00 FC` = `FC:0000` = file `0x3C0000`, which decompresses to 22,528 bytes = 704 SNES 4bpp tiles of recognizable tileset graphics (terrain, slopes, stars). **[verified]**

## 4. Room data format

This is the heart of the editor. Each room pointer leads to a block laid out as: a fixed misc/music header, a graphics header, a sprite-palette header, entrance and door records, six small "sections", a width/height word pair, and finally one HAL-compressed blob containing the tilemap *and* the sprite list. The walkthrough below uses Green Greens room 1 at `0x3020EF`; every byte shown was read from your ROM. **[verified]**

### 4.1 Annotated dump of a complete room header

```
3020EF: 00 00 0F 00 01 01 13 01 00 00 00 00 00 00 00 00 00   ← misc/music header (17 bytes)
302100: FF FF FF FF FF                                       ← padding/terminator
302105: 01 AA 08 6A 6A EA 00 2B 40 40 C9 09 0D 0E 00 00      ← graphics header
302115: 00 00 00 55 4E 57 15 03                                (24 bytes)
30211D: FF                                                   ← terminator
30211E: 99 01 01 01 01 01 99                                 ← sprite palette header (7 bytes)
302125: 2B 00 2A 00                                          ← main entrance
302129: 18 00 02 00 80 01 78 00 9C 00 6C 00 03 00 00         ← door record(s)
302138: FF                                                   ← end of door list
302139: 00 FF C0 00 78 00 FF FF                              ← section 0 payload
302141: 04 00 FF FF                                          ← sections 1–5: "04 00 FF FF"
...                                                            (relative-pointer skip pattern)
302155: EF 3D                                                ← relative ptr (next room's data)
302157: 64 00 08 00                                          ← WIDTH = 0x64 (100), HEIGHT = 8
30215B: 2E 00 02 A0 9B 49 E4 5F 00 ...                       ← HAL-compressed level blob
```

### 4.2 Misc / music header (17 bytes)

Byte offsets within the header, per the Super Famicom Development Wiki (defaults shown are this room's values):

| Offset | Value here | Meaning |
|---|---|---|
| `+0x02` | `0F` | Song ID played in this room (`0F` = Green Greens). |
| `+0x06` | `13` | Background scroll rate (`99` disables parallax scrolling). |
| `+0x08` | `00` | Sprite darkening. |
| `+0x09`, `+0x0A` | `00`, `00` | Background horizontal / vertical shift. |
| `+0x0C` | `00` | Entrance style (`76` = enter on a Warp Star). |

The music byte is also the practical way the community *finds* rooms: search the ROM for the 17-byte header pattern and match the song byte to what you hear in-game. A song-ID reference table (about 65 entries, `00`–`41`) is on the Super Famicom wiki page cited at the end; highlights: `0F` Green Greens, `10` Float Islands, `11` Bubbly Clouds, `07` King Dedede, `2D` Marx battle, `31` Arena.

### 4.3 Graphics header (24 bytes)

From the annotated dump, the header `01 AA | 08 6A 6A EA | 00 | 2B 40 40 C9 09 0D 0E | 00…00 | 55 | 4E 57 15 03` decomposes into three functional groups. Bytes 2–5 (`08 6A 6A EA`) select the **level-object tileset and its palette** — the floors, blocks and slopes, including their collision behavior, so swapping these swaps terrain *behavior* too. Bytes 7–13 (`2B 40 40 C9 09 0D 0E`) select the **background graphics and palette**. The last four bytes (`4E 57 15 03`) are four **sprite GFX slot assignments**, which determine which enemy graphics can display correctly in the room; an enemy placed in a room whose slots don't include its graphics appears as garbage tiles. The clean editor approach is to treat each header as an indivisible "theme" preset, let users copy a whole theme from another room, and validate sprite placements against the four sprite-GFX slots.

### 4.4 Entrance and doors

The main entrance is four bytes: Kirby's X position, X screen number, Y position, Y screen number (`2B 00 2A 00` here). Each door record that follows is 15 bytes:

```
RR RH  XX XS YY YS  DX DS DY DV  (+ trailing/flag bytes, FF ends the list)
RR RH  = destination room index (16-bit; RH is the high byte for rooms ≥ 256)
XX XS YY YS = door trigger position: x, x-screen, y, y-screen
DX DS DY DV = where Kirby appears in the destination room (same layout)
```

In the example, `18 00` is preamble for the record `02 00 | 80 01 78 00 | 9C 00 6C 00`, i.e. a door leading to room `0x0002`, trigger at x=`0x80` on horizontal screen 1, y=`0x78`, exit placement x=`0x9C`, y=`0x6C`. Doors are the most fragile thing to edit: destinations are raw room indices into the 319-entry table, and there is no validity check in the game — your editor should validate every door target and warn about dangling ones.

### 4.5 Sections and the width/height words

After the door list, the room continues with six small variable-length "sections". Each section begins with a 16-bit **relative pointer**: add it to the section's own address to land on the next section. To reach the tilemap, follow the documented recipe — take the room base address, add `0x3A`, hop six sections, then add 2:

```
0x3020EF + 0x3A = 0x302129    (start of section walk)
section 0 @302129: rel 0x0018 → 302141
sections 1–5: rel 0x0004 each → 302145, 302149, 30214D, 302151, 302155
+2 → 0x302157: width = 0x0064 (100), height = 0x0008
```

Both words verified against the decompressed data size below. **[verified]** The sections you hop over hold per-room gameplay metadata (the `04 00 FF FF` pattern is an *empty* section: pointer 4, no payload); most rooms leave several empty. Document-and-preserve is the right editor strategy: keep them as opaque byte ranges and round-trip them untouched.

### 4.6 The compressed level blob — tilemap + sprites in one stream

Immediately after the height word (at `0x30215B` here) sits a single HAL-compressed stream. For this room it is 260 compressed bytes that inflate to **1,650 bytes**, which decomposes exactly as the community docs describe: **[verified]**

```
bytes 0      .. W*H-1    : tile ID low bytes   (800 bytes, row-major, y*W + x)
bytes W*H    .. 2*W*H-1  : tile ID high bytes  (800 bytes, same order)
bytes 2*W*H  .. end      : sprite list          (50 bytes here)
```

Combining the planes gives a 16-bit tile ID per cell: `tile = low[i] | (high[i] << 8)`. These IDs index the room's *metatile* set (the level-object tileset selected in the graphics header), which carries both the 16×16 appearance and the collision type. Rendering row 7 of this room's decoded grid immediately shows ground tiles (`0x0C3/0x0C4/0x0C7` slope pieces, `0x118` decorations) over empty `0x000` sky — the data is human-legible the moment you decode it.

The sprite list is appended after the two planes: **6-byte records, terminated by `FF FF`**, with no count prefix. **[verified]**

```
BB SS XX XS YY YS
BB = behavior/variant (often 00; for ability stars and items it selects the contents)
SS = sprite/enemy ID
XX, XS = x position, horizontal screen number
YY, YS = y position, vertical screen number
```

Green Greens room 1 decodes to eight sprites, matching what you see in-game: a door-star object (`72`), a Warp Star (`75`), a Bronto Burt (`1A` at x=`2D`, screen 2 — this exact record, `00 1A 2D 02 74 00`, is the worked example in the community docs and sits in the decompressed stream precisely where predicted), two more flyers (`0A`), a Waddle Dee (`01`), a Waddle Doo (`02`), and a Mushroom item (`06` with behavior `10`). **[verified]**

A practical historical note: this room's sprite bytes happen to be stored as a *literal run* inside the compressed stream, which is why early hackers could find and hex-edit them at raw file offset `0x302238` without understanding the compression. Your editor must not rely on that accident — always decompress, edit, recompress.

### 4.7 Sprite / enemy ID reference

The full community-built ID table (about 175 entries) is on the Super Famicom Development Wiki page cited at the end; your editor should ship it as a data file. A working subset for orientation: `01` Waddle Dee, `02` Waddle Doo, `03` Sir Kibble, `0D` Gordo, `12` Knuckle Joe, `18` Scarfy, `1A` Bronto Burt, `20` Blade Knight, `26` Birdon, `28` Rocky, `29` Chilly, `2C` T.A.C.; mid-bosses `42`–`47` (Karate Man, Poppy Bros. Sr., Chef Kawasaki, Frosty, Bonkers, Bugzzy); bosses `50` Whispy Woods, `53` Kracko, `54` King Dedede, `55` Dyna Blade, `5A` Meta Knight, `60` Nova's heart; items `80` 1-Up, `81` invincibility candy, `82` Maxim Tomato, `90`–`95` ability selectors; mechanics `75` Warp Star, `77` vertical autoscroll, `78` mine-cart trolley, `7A` launch cannon.

## 5. Graphics and palettes

SNES background graphics are 8×8 tiles in **4bpp planar** format, 32 bytes per tile: rows interleave bitplanes 0/1 in the first 16 bytes and planes 2/3 in the last 16. Decoding in Python:

```python
def decode_4bpp_tile(t):           # t: 32 bytes -> 8x8 array of 0..15
    out = [[0]*8 for _ in range(8)]
    for y in range(8):
        b0, b1 = t[y*2], t[y*2+1]
        b2, b3 = t[16+y*2], t[16+y*2+1]
        for x in range(8):
            s = 7 - x
            out[y][x] = ((b0>>s)&1) | (((b1>>s)&1)<<1) | (((b2>>s)&1)<<2) | (((b3>>s)&1)<<3)
    return out
```

Palettes are the standard SNES 15-bit format, one little-endian word per color: `0BBBBBGG GGGRRRRR`, scaled to 8-bit with `c8 = (c5 << 3) | (c5 >> 2)`. Both tile chunks and palettes referenced from the `FF:0596` table are HAL-compressed; the chunk at `FC:0000` (file `0x3C0000`) was decompressed during this session to 704 valid tiles to confirm the whole pipeline. **[verified]**

Between raw 8×8 tiles and the 16-bit tilemap IDs sits a **metatile (TSA) layer**: each level-object ID maps to four 8×8 tile references (with palette and flip bits) plus a collision type. The community has located fragments of it (`0x377D17` is noted as TSA data affecting Green Greens) but it is not fully mapped — this is the main reverse-engineering work left for a fully graphical editor, and §8 explains how to finish it quickly with a debugger. Until then, an editor can already be highly usable by capturing each room's rendered metatiles from an emulator's tilemap viewer and using those captures as a stamp palette.

## 6. HAL compression — the format everything passes through

Tilemaps, sprite lists, tile graphics, and palettes are all wrapped in HAL Laboratory's LZ-family format, the same one used across EarthBound, Kirby's Dream Course, and Kirby's Dream Land 3. The in-game decompressor lives at `JSL 00889A` (search the ROM for `22 9A 88 00` to find every call site and, near each, the load of a source pointer — a systematic way to discover compressed data your editor doesn't know about yet).

The stream is a sequence of chunks, each starting with a command byte split into a 3-bit command and a 5-bit length; the stream ends at a `0xFF` byte. Decompressed size is capped at 64 KiB by design.

| Command | Meaning |
|---|---|
| `000` | Literal: copy the next `len+1` bytes to output. |
| `001` | 8-bit RLE: next byte, repeated `len+1` times. |
| `010` | 16-bit RLE: next two bytes, repeated `len+1` times (outputs 2×(len+1) bytes). |
| `011` | Incrementing run: next byte, then +1 each, `len+1` bytes total. |
| `100` | Back-reference: copy `len+1` bytes from absolute offset (next 16-bit **big-endian** word) in the *decompressed* output. |
| `101` | Back-reference with each byte bit-reversed. |
| `110` | Back-reference reading backwards from the offset. |
| `111` | Extended header: real command = bits 4–2, 10-bit length = bits 1–0 + next byte. |

Worked example from this room's actual stream **[verified]**: it opens `2E 00` (`001`, len `0x0E`+1 → fifteen `00` bytes — empty sky), then `02 A0 9B 49` (`000`, four literal tile bytes), then `E4 5F 00` (`111` extended: real command `001`, length `0x05F`+1 = 96 → ninety-six more `00` bytes). The 6.35:1 ratio on this room is typical for tilemaps.

Do not write this codec from scratch — use **exhal/inhal** by Devin Acker (github.com/devinacker/exhal, MIT-licensed C, ~one file). `exhal rom offset out` decompresses; `inhal -n in out` produces a compressed blob; `compress.c` embeds directly in your editor, and the repo's `sniff` tool brute-scans a ROM for compressed data. Both were built and run against your ROM during this analysis. Two gotchas: inhal's optimal mode often beats HAL's original compressor (your data may shrink, which is free slack), and *always* verify `decompress(compress(x)) == x` in your save path.

## 7. The save pipeline — where editors live or die

Reading a ROM is easy; writing one back safely is the actual engineering. Recompressed data almost never matches the original size, so a naive in-place overwrite either truncates or tramples the next room. The robust pipeline, in order: serialize the room (rebuild planes + sprite list, re-emit headers and preserved opaque sections), compress with inhal, then place. If the new blob fits in the original slot, write it there and pad the remainder with `0xFF`. If not, write it to managed **free space** and update the 4-byte room pointer at `0x3F009A + 4*index` (and any door-related relative pointers you disturbed). Then recompute the checksum pair at `0x7FDC/0x7FDE`, and verify by round-trip decompression.

For free space, scan for long `0xFF`/`0x00` runs at bank tails to seed a free-list, and reclaim a room's original slot when you relocate it. If you ever exhaust the 4 MB image, SA-1 supports up to 8 MB ROMs, but expanding means touching Super MMC bank mapping — treat that as a last resort, and study how Lunar Magic-style tools or the SA-1 community (Vitor Vilela's sa1-pack work is the best reference) handle it.

For distribution, never ship modified ROMs. Ship **BPS patches** (preferred — they hash the source ROM, which prevents the classic "wrong base ROM" support nightmare; the Floating IPS / `flips` tool makes them) or IPS as a fallback. This is also the community-legal norm: patches contain only your changes, not Nintendo's copyrighted data.

## 8. Filling the gaps: a debugging workflow

Three areas remain partially documented: the full metatile/TSA format, the exact semantics of the six per-room sections, and palette table organization. The fastest route to each is dynamic analysis rather than static staring. In **Mesen 2**, open the ROM, set an execution breakpoint at the decompressor (`00889A`), and enter a room: each break hands you a source pointer (what's being decompressed) and, after stepping out, a destination (where it lands in RAM/VRAM). Decompressed tilemaps land in WRAM, where a write-breakpoint on a tile you then walk past leads you straight into the collision lookup code — and from there to the metatile tables. The Tilemap and Sprite viewers show you the final assembled result to diff against. Log findings as you go; the existing community docs were built exactly this way, twenty years more slowly.

## 9. Suggested editor architecture

A clean structure that has worked for comparable editors (Lunar Magic for SMW, Eggvine for Yoshi's Island, MApRX for the DS remake of this very game):

```
rom_io        file load/save, hash identification, checksum fix, free-space manager
codec         exhal/inhal bindings (or compress.c linked in), round-trip self-test
tables        master/room/level/gfx pointer tables, address conversion (PC ↔ C0+)
model         Room {misc_header, gfx_header, doors[], sections[], W, H,
                    tilemap[W*H] u16, sprites[]}  — opaque-preserve unknown bytes
render        4bpp tile decoder, palette decoder, metatile assembler (or emulator
              screenshot stamps until TSA is fully mapped)
ui            tile canvas (paint/rect/flood), sprite layer (drag-drop, ID picker fed
              by the enemy table), door editor with target validation, theme copier
export        BPS/IPS patch writer
```

Stack-wise, anything works; the comfortable picks are Python + PyQt/Pillow for fast iteration (calling the exhal binaries) or C++/Qt / C# WinForms for the classic ROM-tool feel with `compress.c` linked in. Sequence the build so each milestone is independently useful: a room *viewer* first (pointer walk → decompress → grid render with per-ID colors), then sprite display and editing (small data, low risk, immediately fun), then tile painting with the full save pipeline, then doors and headers, and only then graphics-level features. A viewer alone — which the verified findings in this document fully specify — is a weekend project and validates everything else.

## 10. Practical and legal notes

Build the editor to operate on a user-supplied ROM and distribute only your tool and BPS/IPS patches; don't redistribute the ROM or extracted assets. Test patched ROMs in Mesen 2 or ares for SA-1 accuracy. Keep the original file untouched and work on copies — one bad pointer write can corrupt all 319 rooms' worth of trust in your save code, so add an "open + verify hash + immediately re-save + binary diff = empty" self-test early.

## 11. Sources and further reading

The room/header/sprite/door format, music and sprite ID tables: Super Famicom Development Wiki, "Kirby Super Star" (wiki.superfamicom.org/kirby-super-star, research credited to Proto K and darklink). Compression format and tools: Devin Acker's exhal/inhal (github.com/devinacker/exhal) and the format notes in the same wiki page; the in-game routine address comes from exhal's gamenotes.txt. Animation data ROM map and general game page: Data Crystal (datacrystal.tcrf.net/wiki/Kirby_Super_Star and …/ROM_map). ROM identification data: superfamicom.org's database entry for Hoshi no Kirby Super Deluxe. SNES platform references: the 65816, registers, and memory-mapping documents on wiki.superfamicom.org; SA-1 details in the SNESdev community's SA-1 documentation (fullsnes / SNESdev wiki). For inspiration on UX, MApRX — the level editor for Kirby Super Star Ultra (DS) — demonstrates a proven workflow (extract map → edit → rebuild ROM) for nearly identical level semantics.
