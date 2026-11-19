use xui::prelude::*;

fn main() -> Result<()> {
    // The first argument, if any, is the ROM to open; otherwise the usual
    // places are searched (see `kss_core::Rom::locate`).
    let rom = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    kss_editor::ui::run(rom)
}
