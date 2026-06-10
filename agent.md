# lazykss

## Project Introduction
`lazykss` is a Rust-based tool and library for parsing, viewing, and potentially editing level data from the SNES game *Kirby Super Star* (specifically the US LoROM SA-1 release, `Kirby Super Star (USA).sfc`).
The application is built using the `eframe` / `egui` GUI framework and features:
- A flat ROM address mapper (`snes_to_offset`).
- A custom LZ-family decompressor for HAL Laboratory compression (`src/compression.rs`).
- Level and room parsing, including room headers, graphics header themes, door records, entrance coordinates, and sprite tables (`src/level.rs`).
- A graphical user interface to explore rooms, scroll through levels, view room headers, render tile graphics and palettes, and display tilemaps/sprites/entrances (`src/main.rs`).

## Technical Documentation Reference
For detailed information about the SNES ROM structure, SA-1 chip mapping, master pointer tables, room data headers, and the HAL decompression format, please refer to the main technical documentation file:
- [KSS_Level_Editor_Documentation.md](KSS_Level_Editor_Documentation.md)

## Instructions for AI Agents & Bots
To maintain code quality and ensure stability, any AI bot or agent working on this repository must adhere to the following rules:
1. **Always Verify Compilation**: Before completing any task or submitting changes, run `cargo check` to verify that the project compiles cleanly.
2. **No Warnings Allowed**: Ensure that `cargo check` produces **zero warnings** and zero errors. If any new unused code, dead code, or warnings are introduced, resolve them before finishing the task (e.g., using `#[allow(dead_code)]` or fixing the implementation).
3. **Verify Tests**: Run `cargo test` to ensure that all existing and new unit tests pass successfully.
