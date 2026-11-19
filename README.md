# lazykss

A level editor for *Kirby Super Star* (SNES, US release). This first version
covers the four stages of Spring Breeze: it shows each room as the game draws
it, with its tiles, background and object sprites decoded from your ROM, and
lets you paint tiles, place and move objects, edit doors, and save a new ROM.

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

The GUI is built on [xui](https://github.com/va1erian/xui).

## Licence

MIT, see [LICENSE](LICENSE). The licence covers this code only: *Kirby Super
Star* and its data belong to their owners.
