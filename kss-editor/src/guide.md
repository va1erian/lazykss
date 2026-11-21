# lazykss user guide

A level editor for *Kirby Super Star* (SNES, US). It edits the rooms of
**Spring Breeze** and **The Great Cave Offensive** and saves a new ROM.

[Getting started](#getting-started) ·
[The window](#the-window) ·
[Moving around](#moving-around) ·
[Tiles](#tiles) ·
[Objects](#objects) ·
[Doors and warp stars](#doors-and-warp-stars) ·
[Treasures](#treasures) ·
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

- **Room browser** (left): the stages of Spring Breeze, The Great Cave
  Offensive (81 rooms) and *Other rooms (unverified)*. Each room shows its size in cells.
  A `*` marks a room, or a group, with unsaved changes. The first room of
  each stage is its title card.
- **Map** (centre): the room as the game draws it, with its background,
  tiles and object sprites.
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
| Scroll | Mouse wheel; Shift+wheel scrolls sideways |
| Pan | Drag with the middle button |
| Zoom | Ctrl+wheel, the toolbar, or keys 1 to 4 (1x to 4x) |

Most rooms are wide and short: when a room does not scroll vertically, the
wheel scrolls sideways.

## Tiles

A room is a grid of **cells of 24x24 pixels**. Each cell holds one metatile
from the room's tileset.

- Pick a tile in the **Tiles** tab, or right-click a cell on the map with a
  tile tool (Alt+click also works with Paint). The tab shows the tile id and
  its **attribute** (collision class: `00` empty, `50` solid ground...).
- **Paint** (B): click or drag. A drag is one undo step.
- **Rect** (R): drag a rectangle, release to fill it.
- **Fill** (G): flood fill of the connected cells with the same tile.

Tile `000` is fully transparent, not "nothing": it is an ordinary tile.

**Star and bomb blocks** (attribute `70`-`7F`) only break in game when the
room lists their cell. The editor does this for you on save: blocks you
painted are added, blocks you painted over are removed. No action needed.

## Objects

Enemies, items, stars and markers. Each has an **ID** (what it is), a
**variant** (which version, or a number its kind uses) and a position in
pixels from the room's top-left corner.

- **Select** (V): click an object to select it and drag to move it. Arrow
  keys nudge by 1 pixel, Shift+arrows by 8.
- **Object** (O): click the map to place a new object of the ID and variant
  set under *New object* in the Objects tab. **Add** places one in the
  middle of the view.
- **Delete** or Backspace removes the selected object. Esc deselects.
- Edit the ID, variant and position of the selected object in the tab.

Things to know:

- The list shows ID and variant in **hex**; the number fields take
  **decimal**. A treasure chest is `B8` in the list and 184 in the field.
- Positions are kept inside the room.
- Objects the editor cannot draw (bosses, stage intros, a few others) appear
  as labelled boxes. They work in game all the same.
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
- Stored `7FFF` is the stage's **goal door** (it ends the stage; its door
  star has variant 16). Other stored values from `4000` up are special
  codes whose meaning is unknown: leave them alone.
- A door leading outside its level, or to a room that does not exist, gets a
  warning. **The game does not check doors**: a bad one can crash it.
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

## Room settings

The **Room** tab:

- **Music**: the song id the room plays, with its name when known.
- **Size**: width and height in cells (up to 512x128, and at most 30,000
  cells). **Resize** keeps the top-left cells; new cells are empty (tile
  0). Objects and doors left outside the new size stay in the lists: the
  game ignores them, so move or delete them.
- **Room header**: shown for reference, read-only.

## Undo

Ctrl+Z undoes, Ctrl+Y (or Ctrl+Shift+Z) redoes. **Each room has its own
history**: undo acts on the room on screen only. A paint stroke, a drag and
a door deletion are each one step. History survives saving, but not opening
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
- **Open ROM** and **Exit** warn once about unsaved changes; choose them
  again to discard the changes.

**Always test a saved ROM in an emulator** before building on it.

## Limitations

- Only Spring Breeze and The Great Cave Offensive are named and tested.
  Rooms under *Other rooms (unverified)* open and save, but nothing about them has been
  checked in game.
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
| V B R G O | Select, Paint, Rect, Fill, Object tool (map focused) |
| 1-4 | Zoom 1x to 4x (map focused) |
| Arrows / Shift+arrows | Nudge the selected object 1 / 8 px |
| Delete, Backspace | Delete the selected object, else the selected door |
| Esc | Deselect, cancel a rectangle |
| Right-click, Alt+click | Pick the tile under the pointer (tile tools) |
