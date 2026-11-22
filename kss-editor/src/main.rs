// A release build on Windows is a GUI app: no console window opens with it.
// Debug builds keep the console, where panics and logs show.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use xui::prelude::*;

fn main() -> Result<()> {
    // The first argument, if any, is the ROM to open; otherwise the usual
    // places are searched (see `kss_core::Rom::locate`).
    let rom = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    kss_editor::ui::run(rom)
}
