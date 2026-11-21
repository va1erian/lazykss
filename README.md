# lazykss

A level editor for *Kirby Super Star* (SNES, US release). It covers the four
stages of Spring Breeze and The Great Cave Offensive: it shows each room as
the game draws it, with its tiles, background and object sprites decoded from
your ROM, and lets you paint tiles, place and move objects, add, edit and
delete doors and warp stars, place the cave's treasure chests (the Treasures
tab lists all 60 treasures and where each one is), and save a new ROM.

The ROM is not included and must not be added to this repository. Supply your
own headerless US dump (`SHA-1 cca1e67fa230bf4f60154f5ea55d06568229dc5d`).

```text
cargo run -p kss-editor -- "path/to/Kirby Super Star (USA).sfc"
```

Save never writes over the file you opened, nor over an edited file from an
earlier session: it writes `<name> (edited).sfc` next to the ROM, or
`(edited 2)` and so on when that exists, unless Save As picks another path.

- [KSS_Level_Editor_Documentation.md](KSS_Level_Editor_Documentation.md) —
  the data formats, and how each statement was verified.
- [agent.md](agent.md) — repository layout and the rules for changes.
- [docs/emulator-harness.md](docs/emulator-harness.md) — the scripted
  emulator used to work the formats out, and how to use it.

The GUI is built on [xui](https://github.com/va1erian/xui).

## Releases

Pushing a tag `vX.Y.Z` (or `vX.Y.Z-suffix`, published as a pre-release) runs
`.github/workflows/release.yml`: it tests and builds the editor for Windows
and Linux (x86-64) and attaches the archives, with their SHA-256 sums, to a
GitHub release named after the tag.

```text
git tag v0.2.0-beta1 && git push origin v0.2.0-beta1
```

## Licence

MIT, see [LICENSE](LICENSE). The licence covers this code only: *Kirby Super
Star* and its data belong to their owners.
