# lazykss user guide

A level editor for *Kirby Super Star* (SNES, US). It edits the rooms of
every sub-game played through rooms: **Spring Breeze**, **Dyna Blade**,
**Gourmet Race**, **The Great Cave Offensive**, **Revenge of Meta Knight**,
**Milky Way Wishes** and **The Arena**, and saves a new ROM.

[Getting started](#getting-started) ·
[The window](#the-window) ·
[Moving around](#moving-around) ·
[Tiles](#tiles) ·
[Buffer and stamp](#buffer-and-stamp) ·
[Objects](#objects) ·
[Doors and warp stars](#doors-and-warp-stars) ·
[Treasures](#treasures) ·
[Sub-game objects](#sub-game-objects) ·
[Room settings](#room-settings) ·
[Undo](#undo) ·
[Saving](#saving) ·
[Limitations](#limitations) ·
[Shortcuts](#shortcuts)

## Getting started

You need your own **headerless US ROM** (a 512-byte copier header is removed
on load). The editor opens, in order: the file given on the command line, the
file named by the `KSS_ROM` environment variable, or
`Kirby Super Star (USA).sfc` in the current folder or a parent. Otherwise use
**File > Open ROM** (Ctrl+O).

Your original ROM is never modified unless you pick it yourself in Save As.

## The window

- **Room browser** (left): each sub-game, its levels (stages, courses,
  chapters, planets) and their rooms; The Arena's rooms sit directly under
  it. *Other rooms* holds the guide's theatre, demo and dummy rooms, which
  no sub-game plays. Each room shows its size in cells, and a `*` marks a
  room, or a group, with unsaved changes. Spring Breeze opens expanded.
  The first room of each Spring Breeze stage is its title card; Dyna Blade
  starts in its *second* room (28).
- **Map** (centre): the room as the game draws it, with its background,
  tiles and object sprites. Around it, on a checkerboard, is the
  [buffer](#buffer-and-stamp): scratch space that is never saved.
- **Tabs** (right): Tiles, Objects, Doors, Room, Treasures, and Help
  (this guide; F1 or **Help > User guide** opens it).
- **Status bars** (bottom): the room, the cell and pixel under the pointer,
  the zoom and current tile; below, messages from the last action. Read them
  when an action seems to do nothing: they say why.

The layer strip in the toolbar shows or hides the **BG** (background), **FG**
(tiles), the cell **grid**, **door** triggers and **object** markers. Hiding
a layer never changes the room.

## Moving around

| Action | How |
|---|---|
| Scroll | Mouse wheel; Shift+wheel scrolls the other way |
| Pan | Drag with the middle button |
| Zoom | Ctrl+wheel, the toolbar, or keys 1 to 4 (1x to 4x) |

Most rooms are wide and short: when a room fits the map's height, the wheel
scrolls sideways and **Shift+wheel** scrolls up and down, into the buffer
above and below. In a taller room the wheel scrolls up and down and
Shift+wheel sideways. A room opens scrolled to its top-left corner.

## Tiles

A room is a grid of **cells of 24x24 pixels**. Each cell holds one metatile
from the room's tileset.

- Pick a tile in the **Tiles** tab, or right-click a cell on the map with a
  tile tool (Alt+click also works with Paint). The tab shows the tile id and
  its **attribute** (collision class: `00` empty, `50` solid ground...).
- **Paint** (B): click or drag. A drag is one undo step. **Shift+drag**
  erases [buffer](#buffer-and-stamp) cells.
- **Rect** (R): drag a rectangle, release to fill it.
- **Fill** (G): flood fill of the connected cells with the same tile.
- **Stamp** (S): copy cells and paste them elsewhere; see
  [Buffer and stamp](#buffer-and-stamp).

All tile tools work on the room and on the buffer around it.

Tile `000` is fully transparent, not "nothing": it is an ordinary tile.

**Star and bomb blocks** (attribute `70`-`7F`) only break in game when the
room lists their cell. The editor does this for you on save: blocks you
painted are added, blocks you painted over are removed. No action needed.

## Buffer and stamp

The **buffer** is a band of 16 cells all around the room, drawn on a
checkerboard, with a line at the room's edge. Use it to compose a motif (a
platform, a pipe, a decoration) before putting it in the room.

- Paint, Rect, Fill and tile picking work in the buffer as on the map. A
  rectangle across the room's edge fills both. A flood fill stays on the
  side it starts on.
- **Shift+drag** with Paint empties buffer cells. Room cells cannot be
  emptied: paint them with another tile.
- The buffer is **never saved** and does not mark the room as modified. Its
  edits are undone and redone like the others. Each room has its own buffer,
  so a motif shows in that room's tileset. It is lost when another ROM is
  opened.
- Objects cannot go in the buffer: the Object tool ignores clicks there.

The **Stamp** tool (S) copies a rectangle of cells and pastes it:

- **Right-drag** over cells, in the buffer or in the room, to copy them.
  Empty buffer cells are see-through: pasting leaves what is under them, so
  a motif need not be a rectangle.
- **Click** to paste. The stamp follows the pointer, ghosted, where a click
  would put it.
- **Drag** to repeat it: a copy is pasted each time the pointer moves a
  whole stamp's width or height from where the drag began. The whole drag
  is one undo step.
- The stamp stays when you switch rooms. Its cells are tile numbers, so in a
  room with another tileset they show other tiles; the status bar warns
  when that happens.

## Objects

Enemies, items, stars and markers. Each has an **ID** (what it is), a
**variant** (which version, or a number its kind uses) and a position in
pixels from the room's top-left corner.

- **Select** (V): click an object to select it and drag to move it. Arrow
  keys nudge by 1 pixel, Shift+arrows by 8.
- **Object** (O): click the room to place a new object of the ID and variant
  set under *New object* in the Objects tab (not in the buffer). **Add** places one in the
  middle of the view.
- **Delete** or Backspace removes the selected object. Esc deselects.
- Edit the ID, variant and position of the selected object in the tab.

Things to know:

- The list shows ID and variant in **hex**; the number fields take
  **decimal**. A treasure chest is `B8` in the list and 184 in the field.
- Positions are kept inside the room.
- Objects the editor cannot draw (most bosses, stage intros, invisible
  markers and all of Gourmet Race's signs and foods) appear as labelled
  boxes. They work in game all the same.
- Not every ID and variant pair exists in the game. An unknown one may do
  nothing, misbehave or crash: test in an emulator.

## Doors and warp stars

A **door** is a record in the room's door list: a **trigger** cell, a
**destination room** and an **arrival point** (pixels in that room). Kirby
takes a door by pressing **Up** while standing in its trigger cell. On the
map, a trigger is the box of the cell whose top-left corner is the door's
position.

- **Add door** puts a door on the cell in the middle of the view, with the
  invisible **door star** (object `72`) the game's own doors have at the
  cell's centre. It leads back to the same spot until you change it.
- **Add warp** places a **warp star** (object `75`) and a door record with
  no trigger. The star's variant is the index of that record in the list.
- **Delete** removes the door with its door star and its warp stars, and
  renumbers the warp stars of later doors, in one undo step.
- **Go to destination** opens the destination room centred on the arrival
  point. Use it to check every door you edit.
- With Select, a click on a trigger (or on its door star) selects the door.

Rules and corner cases:

- **Doors count rooms from their level's first room.** Type the real room
  number in *To room*; the editor stores it relative to the level and shows
  the stored value in *Stored*. A room before the level's first room
  cannot be reached and is refused.
- **A door needs a door tile.** Kirby can only take a door whose trigger
  cell holds a door tile (collision attribute `0F`; the Tiles tab shows a
  tile's attribute). *Add door* does not paint one: paint the tileset's door
  tile on the trigger cell. The Doors tab warns when it is missing. Some of
  the game's own doors hide under a breakable block and work once it is
  broken.
- Stored `7FFF` is the stage's **goal door** (it ends the stage; its door
  star has variant 16).
- A stored `4000 + n` leads to the same room as `n`, with a flag whose
  purpose is unknown (two of the game's doors use it). Editing *To room*
  keeps the flag.
- A door to a room that does not exist gets a warning, and a door past its
  level's last room a note: the game follows it (a test door from Dyna
  Blade's room 28 entered Mallow Castle). **The game does not check doors**: a
  bad one can crash it.
- The arrival point is not checked either. Make sure Kirby lands somewhere
  he can stand.
- A room holds at most 255 doors.

## Treasures

The Great Cave Offensive's **60 treasures** are chests (object `B8`) whose
**variant is the treasure number** (0 to 59). The number alone decides what
the chest holds.

- The **Treasures** tab lists every treasure with its value and the room of
  its chest. Click one to jump to its chest.
- **Place with Object tool** arms the Object tool with a chest of the picked
  treasure: click the map to place it.

Corner cases:

- Only chests in the cave's rooms are listed. A chest elsewhere still opens,
  but whether it counts towards the collection is unknown.
- In the original game, treasures 0 (Gold Medal) and 16 (Saucepan) have no
  chest.
- Numbers from 60 up open an empty window. Two chests with the same number
  have not been tested.

## Sub-game objects

Some objects of the other sub-games use their variant for a number. The
Objects list names them:

- **Gourmet Race**: `C0` is a **race sign**: variant 0 starts the race, 16
  is a checkpoint that ends the course (courses 1 and 2), 32 the goal
  (course 3). A course needs a start and a checkpoint or goal: without
  them the race never starts or never ends. `65` is King Dedede's start
  place, `C1` an arrow sign (the variant turns it), `C2` to `D1` are foods
  and `D2` a Maxim Tomato.
- **Revenge of Meta Knight**: `BC` is a **dialogue**: its variant is the
  line (0 to 24) shown, with its speaker, when Kirby comes near. The lines
  are shared by the whole sub-game.
- Ability items (`90`, `91`) and Milky Way Wishes' copy essences (`95`)
  seem to pick the ability with the variant's first hex digit; this has
  not been tested.

## Room settings

The **Room** tab:

- **Music**: the song id the room plays, with its name when known. With
  128 added (bit 7 set, as in Revenge of Meta Knight's dialogue rooms) the
  room starts no song and silences the one playing: the game starts the
  music later from a script.
- **Size**: width and height in cells (up to 512x128, and at most 30,000
  cells). **Resize** keeps the top-left cells; new cells are empty (tile
  0). Objects and doors left outside the new size stay in the lists: the
  game ignores them, so move or delete them. Buffer cells the new size
  covers, or leaves beyond the buffer, are cleared; undo brings them back.
- **Room header**: shown for reference, read-only.

## Undo

Ctrl+Z undoes, Ctrl+Y (or Ctrl+Shift+Z) redoes. **Each room has its own
history**: undo acts on the room on screen only. A paint stroke, a drag,
a stamp drag and a door deletion are each one step. Buffer edits are in the
history too. History survives saving, but not opening
another ROM.

## Saving

- **Save** (Ctrl+S) writes `<name> (edited).sfc` next to your ROM. If that
  file already exists from an earlier session, it writes `(edited 2)`, then
  `(edited 3)`, and so on: an earlier save is never replaced. Later saves in
  the same session go to the same file.
- **Save As** (Ctrl+Shift+S) writes to a file you choose. It is the only
  way to overwrite a file, including your original ROM.
- Only changed rooms are written. The status bar lists each room's new size
  and whether it was **moved**.
- A room that grows no longer fits its old place and is moved to free space
  at the end of a ROM bank. That space is small (about 32 KiB in total): if
  a save fails for lack of room, shrink some edits.
- The ROM checksum is fixed on every save.
- **Open ROM**, **Exit** and the window's close button warn once about
  unsaved changes; do it again to discard the changes.

**Always test a saved ROM in an emulator** before building on it.

## Limitations

- Spring Breeze was played through; the first rooms of Dyna Blade, Gourmet
  Race, The Great Cave Offensive and Revenge of Meta Knight were edited,
  saved and played for a few seconds. Milky Way Wishes and The Arena open,
  render and save, but no edited room of theirs has been run. Their level
  names, and those of the Revenge of Meta Knight chapters, come from what
  the rooms show.
- The level a room belongs to comes from the menu, not from the room: a
  room's header byte 0 is a tag the game ignores.
- Not editable: tilesets, graphics, palettes, the background, collision
  attributes, the room header, scroll and camera settings.
- The background is drawn fixed to the room's corner; in game it scrolls at
  its own rate.
- Animated tiles show their first frame. King Dedede's arena background has
  wrong colours in the editor (not in game).
- New enemies, items and doors are taken from the game's own data: the
  editor cannot create new kinds of objects or behaviour.

## Shortcuts

| Key | Action |
|---|---|
| Ctrl+O | Open ROM |
| Ctrl+S / Ctrl+Shift+S | Save / Save As |
| Ctrl+Z / Ctrl+Y | Undo / Redo |
| F1 | Show this guide |
| V B R G S O | Select, Paint, Rect, Fill, Stamp, Object tool (map focused) |
| 1-4 | Zoom 1x to 4x (map focused) |
| Arrows / Shift+arrows | Nudge the selected object 1 / 8 px (map focused) |
| Delete, Backspace | Delete the selected object, else the selected door (map focused) |
| Esc | Deselect, cancel a rectangle (map focused) |
| Right-click, Alt+click | Pick the tile under the pointer (tile tools) |
| Shift+drag | Erase buffer cells (Paint) |
| Right-drag / click / drag | Copy cells / paste / repeat the stamp (Stamp) |
| Shift+wheel | Scroll the other way (into the buffer above and below a short room) |
