//! Embeds the icon (`assets/kss-editor.ico`, drawn by `tools/make_icon.py`)
//! in the Windows executable, where Explorer and the taskbar find it. The
//! windows show the same resource: see `ui::run`.

fn main() {
    println!("cargo:rerun-if-changed=assets/kss-editor.rc");
    println!("cargo:rerun-if-changed=assets/kss-editor.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // A Windows build without its icon is a broken release: fail loudly.
    if let Err(error) =
        embed_resource::compile("assets/kss-editor.rc", embed_resource::NONE).manifest_required()
    {
        eprintln!("embedding the icon failed: {error}");
        std::process::exit(1);
    }
}
