# lazykss

A level editor for the SNES game *Kirby Super Star* (US release, SA-1). It
covers the four stages of Spring Breeze and The Great Cave Offensive.

## Layout

- `kss-core/` — no UI dependencies. ROM access (`rom.rs`), the HAL codec
  (`compression.rs`), rooms and the save pipeline (`room.rs`), tileset
  rendering (`gfx.rs`), object sprites (`sprites.rs` and the generated
  `sprite_table.rs`), names (`names.rs`), the cave's treasures
  (`treasure.rs`).
  Examples: `render_room` (rooms to PPM), `patch_room` (scripted edits).
- `kss-editor/` — the GUI, on [xui](https://github.com/va1erian/xui).
  `editor.rs` and `history.rs` hold the editing logic and undo, with no UI
  types; `ui/` holds the window, the custom-painted map and tile picker.
- `KSS_Level_Editor_Documentation.md` — the data formats, with what was
  verified and how. Read it before touching `kss-core`.
- `tools/harness/` and `docs/emulator-harness.md` — a scripted, headless
  emulator and the method for answering questions about the game with it.

## Running

```text
cargo run -p kss-editor [rom.sfc]
```

Without an argument the ROM is found through `KSS_ROM`, or as
`Kirby Super Star (USA).sfc` in the current directory or any parent. The ROM
is not part of the repository. Save never writes over the file that was
opened, nor over a file from an earlier session: it writes
`<name> (edited).sfc` next to the ROM (`(edited 2)`, ... when that exists)
unless Save As picks another path.

## Rules for agents

1. `cargo check --all-targets` and `cargo test` must finish with zero
   warnings and zero failures. Tests that need the ROM pass silently without
   it, so run them where the ROM can be found.
2. Check UI changes headlessly: `XUI_LAYOUT_REPORT=1 XUI_SNAPSHOT=target/shots
   cargo run -p kss-editor` writes light and dark PNGs and must report
   `XUI_LAYOUT_WARNINGS:0`. Look at the PNGs.
3. `kss-core/src/sprite_table.rs` is generated: `python tools/harness/sprites.py`
   rewrites it from the running game. Do not edit it by hand or guess
   addresses.
4. A change to the save pipeline is not done until a saved ROM has been
   booted: see [docs/emulator-harness.md](docs/emulator-harness.md), "does a
   saved ROM still work?".
5. When the ROM bytes can be read two ways, run the game and look. That
   guide explains how, step by step, and lists the mistakes already made.
