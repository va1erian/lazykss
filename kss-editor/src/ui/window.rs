//! The window's layout: menu, toolbar, browser, map, side panel, status bars.

use std::path::PathBuf;
use std::rc::Rc;

use xui::Lucide;
use xui::prelude::*;
use xui::widget::FileDialog;
use xui::xui_markdown::MarkdownView;

use super::SharedEditor;
use super::app::{DoorField, EditorApp, Handles, Layer, Msg, ObjField};
use super::keys;
use super::map::MapView;
use super::picker::TilePicker;
use crate::editor::{Editor, Tool};
use kss_core::Rom;

const OPEN: MenuId = MenuId::new(1);
const SAVE: MenuId = MenuId::new(2);
const SAVE_AS: MenuId = MenuId::new(3);
const QUIT: MenuId = MenuId::new(4);
const UNDO: MenuId = MenuId::new(10);
const REDO: MenuId = MenuId::new(11);
const ZOOM_IN: MenuId = MenuId::new(20);
const ZOOM_OUT: MenuId = MenuId::new(21);
const GUIDE: MenuId = MenuId::new(30);

/// The user guide the Help tab shows.
const GUIDE_TEXT: &str = include_str!("../guide.md");

/// The Help tab's position in the side panel.
pub const HELP_TAB: usize = 5;

fn menu_message(id: MenuId) -> Option<Msg> {
    Some(match id {
        OPEN => Msg::OpenRom,
        SAVE => Msg::Save,
        SAVE_AS => Msg::SaveAs,
        QUIT => Msg::Quit,
        UNDO => Msg::Undo,
        REDO => Msg::Redo,
        ZOOM_IN => Msg::ZoomIn,
        ZOOM_OUT => Msg::ZoomOut,
        GUIDE => Msg::ShowGuide,
        _ => return None,
    })
}

fn menu() -> Build<Menu<Msg>, Msg> {
    menu_bar(|m| {
        m.submenu(MenuId::new(100), "&File", |f| {
            f.item(OPEN, "&Open ROM...    Ctrl+O");
            f.item(SAVE, "&Save    Ctrl+S");
            f.item(SAVE_AS, "Save &As...    Ctrl+Shift+S");
            f.separator();
            f.item(QUIT, "E&xit");
        });
        m.submenu(MenuId::new(101), "&Edit", |e| {
            e.item(UNDO, "&Undo    Ctrl+Z");
            e.item(REDO, "&Redo    Ctrl+Y");
        });
        m.submenu(MenuId::new(102), "&View", |v| {
            v.item(ZOOM_IN, "Zoom &in");
            v.item(ZOOM_OUT, "Zoom &out");
        });
        m.submenu(MenuId::new(103), "&Help", |h| {
            h.item(GUIDE, "&User guide    F1");
        });
    })
    .on_select_with(menu_message)
}

/// A number field that reports its value when committed (Enter, a step or
/// leaving the field).
fn number(
    handle: &Handle<NumberField<Msg>>,
    max: f64,
    f: impl Fn(f64) -> Msg + 'static,
) -> Build<NumberField<Msg>, Msg> {
    number_field(0.0, max, 1.0)
        .then(move |n| n.on_commit(move |v| Some(f(v))))
        .bind(handle)
}

/// A square icon-only button.
fn icon_button(icon: Lucide, tip: &str, msg: Msg) -> Build<Button<Msg>, Msg> {
    button("").icon(icon).tooltip(tip).on_click(msg)
}

fn tool_icon(tool: Tool) -> Lucide {
    match tool {
        Tool::Select => Lucide::MousePointer2,
        Tool::Paint => Lucide::Pencil,
        Tool::Rect => Lucide::RectangleHorizontal,
        Tool::Fill => Lucide::PaintBucket,
        Tool::AddObject => Lucide::Box,
    }
}

fn toolbar_row(h: &Handles) -> Layout<Msg> {
    // Checkable strips: the tools behave as a radio group (the app unchecks
    // the others), the layers toggle on their own.
    let mut tools = toolbar();
    for tool in Tool::ALL {
        tools = tools.toggle_item(tool_icon(tool), tool.hint());
    }
    let tools = tools
        .checked(0, true)
        .on_click(|index| {
            // A click past the end (which the strip does not make) picks the last.
            let [.., last] = Tool::ALL;
            Msg::SetTool(Tool::ALL.get(index).copied().unwrap_or(last))
        })
        .bind(&h.tools);

    let mut layers = toolbar();
    for (index, layer) in Layer::ALL.into_iter().enumerate() {
        layers = match layer {
            Layer::Background => {
                layers.toggle_item_with_text(Lucide::Image, "Show the background layer", "BG")
            }
            Layer::Foreground => layers.toggle_item_with_text(
                Lucide::Layers,
                "Show the foreground (tile) layer",
                "FG",
            ),
            Layer::Grid => layers.toggle_item(Lucide::Grid3x3, "Show the cell grid"),
            Layer::Doors => layers.toggle_item(Lucide::DoorOpen, "Show door triggers"),
            Layer::Objects => layers.toggle_item(Lucide::Eye, "Show object markers"),
        };
        layers = layers.checked(index, layer != Layer::Grid);
    }
    let layers = layers
        .on_click(|index| {
            let [.., last] = Layer::ALL;
            Msg::Layer(Layer::ALL.get(index).copied().unwrap_or(last))
        })
        .bind(&h.layers);

    row().padding(6).gap(8).children((
        row().gap(2).children((
            icon_button(Lucide::FolderOpen, "Open a ROM (Ctrl+O)", Msg::OpenRom).width(36),
            icon_button(Lucide::Save, "Save to the edited copy (Ctrl+S)", Msg::Save).width(36),
            icon_button(
                Lucide::SaveAll,
                "Save to a chosen file (Ctrl+Shift+S)",
                Msg::SaveAs,
            )
            .width(36),
        )),
        row().gap(2).children((
            icon_button(Lucide::Undo2, "Undo (Ctrl+Z)", Msg::Undo)
                .bind(&h.undo)
                .width(36),
            icon_button(Lucide::Redo2, "Redo (Ctrl+Y)", Msg::Redo)
                .bind(&h.redo)
                .width(36),
        )),
        tools,
        layers,
        row().gap(6).align(Align::Center).children((
            icon_button(Lucide::ZoomOut, "Zoom out (Ctrl+wheel)", Msg::ZoomOut).width(36),
            label("2x").bind(&h.zoom_label),
            icon_button(Lucide::ZoomIn, "Zoom in (Ctrl+wheel)", Msg::ZoomIn).width(36),
        )),
    ))
}

fn tiles_tab(h: &Handles, editor: &SharedEditor) -> Layout<Msg> {
    let shared = Rc::clone(editor);
    column().padding(8).gap(6).children((
        label("").bind(&h.tile_info),
        build(move |ui| TilePicker::new(ui, shared, Msg::PickTile))
            .bind(&h.picker)
            .fill(1),
    ))
}

fn objects_tab(h: &Handles) -> Layout<Msg> {
    let f = &h.object_fields;
    let n = &h.new_object_fields;
    column().padding(8).gap(6).children((
        list()
            .column("ID", 36)
            .column("Name", Fill)
            .column("Var", 36)
            .column_right("X", 48)
            .column_right("Y", 40)
            .on_select(Msg::ObjectSelect)
            .bind(&h.objects)
            .fill(1),
        row().gap(6).children((
            button("Add")
                .tooltip("Add an object in the middle of the view")
                .on_click(Msg::ObjectAdd),
            button("Delete")
                .tooltip("Delete the selected object (Del)")
                .on_click(Msg::ObjectDelete),
        )),
        group(
            "Selected object (values in decimal)",
            grid([Track::Auto, Track::Fill(1), Track::Auto, Track::Fill(1)])
                .gap(6)
                .children((
                    label("ID").align(Align::Center),
                    number(&f[0], 255.0, |v| Msg::ObjectField(ObjField::Id, v)),
                    label("Variant").align(Align::Center),
                    number(&f[1], 255.0, |v| Msg::ObjectField(ObjField::Variant, v)),
                    label("X").align(Align::Center),
                    number(&f[2], 65535.0, |v| Msg::ObjectField(ObjField::X, v)),
                    label("Y").align(Align::Center),
                    number(&f[3], 65535.0, |v| Msg::ObjectField(ObjField::Y, v)),
                )),
        ),
        group(
            "New object (Object tool)",
            grid([Track::Auto, Track::Fill(1), Track::Auto, Track::Fill(1)])
                .gap(6)
                .children((
                    label("ID").align(Align::Center),
                    number(&n[0], 255.0, |v| Msg::NewObjectField(ObjField::Id, v)),
                    label("Variant").align(Align::Center),
                    number(&n[1], 255.0, |v| Msg::NewObjectField(ObjField::Variant, v)),
                )),
        ),
    ))
}

fn doors_tab(h: &Handles) -> Layout<Msg> {
    let f = &h.door_fields;
    column().padding(8).gap(6).children((
        list()
            .column("#", 28)
            .column("To", 84)
            .column("Trigger", Fill)
            .column("Arrive at", 90)
            .on_select(Msg::DoorSelect)
            .bind(&h.doors)
            .fill(1),
        row().gap(6).children((
            button("Add door")
                .tooltip("Add a door (and its door star) in the middle of the view")
                .on_click(Msg::DoorAdd),
            button("Add warp")
                .tooltip("Add a warp star and the door record it uses")
                .on_click(Msg::WarpAdd),
            button("Delete")
                .tooltip("Delete the selected door, its door star and its warp stars")
                .on_click(Msg::DoorDelete),
        )),
        group(
            "Selected door",
            grid([Track::Auto, Track::Fill(1), Track::Auto, Track::Fill(1)])
                .gap(6)
                .children((
                    label("To room").align(Align::Center),
                    number(&f[0], 65535.0, |v| Msg::DoorField(DoorField::DestRoom, v)),
                    label("Stored").align(Align::Center),
                    number(&f[1], 65534.0, |v| Msg::DoorField(DoorField::Raw, v)),
                    label("Trigger X").align(Align::Center),
                    number(&f[2], 65535.0, |v| Msg::DoorField(DoorField::X, v)),
                    label("Trigger Y").align(Align::Center),
                    number(&f[3], 65535.0, |v| Msg::DoorField(DoorField::Y, v)),
                    label("Arrive X").align(Align::Center),
                    number(&f[4], 65535.0, |v| Msg::DoorField(DoorField::DestX, v)),
                    label("Arrive Y").align(Align::Center),
                    number(&f[5], 65535.0, |v| Msg::DoorField(DoorField::DestY, v)),
                )),
        ),
        label("").bind(&h.door_note),
        row().child(button("Go to destination").on_click(Msg::DoorGoto)),
    ))
}

fn treasures_tab(h: &Handles) -> Layout<Msg> {
    column().padding(8).gap(6).children((
        list()
            .column("#", 28)
            .column("Treasure", Fill)
            .column_right("Value", 70)
            .column("Room", 70)
            .on_select(Msg::TreasureSelect)
            .bind(&h.treasures)
            .fill(1),
        label("").bind(&h.treasure_note),
        row().child(
            button("Place with Object tool")
                .tooltip("Click on the map to place a chest of the picked treasure")
                .on_click(Msg::TreasurePlace),
        ),
    ))
}

fn room_tab(h: &Handles) -> Layout<Msg> {
    column().padding(8).gap(6).children((
        group(
            "Music",
            grid([Track::Auto, Track::Fill(1)]).gap(6).children((
                label("Song id").align(Align::Center),
                number(&h.music, 255.0, Msg::Music),
                label("Name").align(Align::Center),
                label("").bind(&h.music_name),
            )),
        ),
        group(
            "Size (cells of 24x24 pixels, at most 30000)",
            column().gap(6).children((
                grid([Track::Auto, Track::Fill(1), Track::Auto, Track::Fill(1)])
                    .gap(6)
                    .children((
                        label("Width").align(Align::Center),
                        number_field(1.0, 512.0, 1.0)
                            .then(|n| n.on_commit(|v| Some(Msg::RoomWidth(v))))
                            .bind(&h.size_fields[0]),
                        label("Height").align(Align::Center),
                        number_field(1.0, 128.0, 1.0)
                            .then(|n| n.on_commit(|v| Some(Msg::RoomHeight(v))))
                            .bind(&h.size_fields[1]),
                    )),
                row().child(
                    button("Resize")
                        .tooltip("Keeps the top-left cells; new cells are empty")
                        .on_click(Msg::ResizeRoom),
                ),
            )),
        ),
        label("Room header (read-only)").caption(),
        list().column("Offset  bytes", Fill).bind(&h.head).fill(1),
    ))
}

/// The user guide, read-only. Links to its own headings scroll the view;
/// it has no other links to follow.
fn help_tab(h: &Handles) -> Layout<Msg> {
    column().children((build(
        |ui| Ok(MarkdownView::new(ui, Rect::default())?.markdown(GUIDE_TEXT)),
    )
    .bind(&h.guide)
    .fill(1),))
}

/// Builds the window and returns the app, with the first room open.
pub fn create(ui: &mut Ui<Msg>, rom: Option<PathBuf>, editor: SharedEditor) -> Result<EditorApp> {
    let h = Handles::default();

    let map_editor = Rc::clone(&editor);
    ui.root(
        column().children((
            menu(),
            toolbar_row(&h),
            row()
                .padding(6)
                .gap(6)
                .children((
                    tree_view()
                        .on_select(Msg::BrowserSelect)
                        .then(|t| t.on_toggle(|id, open| Some(Msg::BrowserToggle(id, open))))
                        .bind(&h.tree)
                        .width(250),
                    build(move |ui| MapView::new(ui, map_editor, Msg::Map))
                        .bind(&h.map)
                        .fill(1),
                    tabs()
                        .page("Tiles", tiles_tab(&h, &editor))
                        .page("Objects", objects_tab(&h))
                        .page("Doors", doors_tab(&h))
                        .page("Room", room_tab(&h))
                        .page("Treasures", treasures_tab(&h))
                        .page("Help", help_tab(&h))
                        .bind(&h.tabs)
                        .width(350),
                ))
                .fill(1),
            status_bar(&["", "", ""]).bind(&h.info),
            status_bar(&["Ready"]).bind(&h.message),
        )),
    )?;

    ui.on_key(keys::shortcut);

    let open_dialog = FileDialog::open_file(ui, "Open ROM")?
        .filter("SNES ROM", &["sfc", "smc"])
        .on_accept(|path| Some(Msg::RomChosen(path)))
        .on_cancel(|| None);
    let save_dialog = FileDialog::save_file(ui, "Save As")?
        .filter("SNES ROM", &["sfc", "smc"])
        .on_accept(|path| Some(Msg::SaveAsChosen(path)))
        .on_cancel(|| None);

    let mut app = EditorApp::new(editor, h, open_dialog, save_dialog);
    app.start(rom);
    Ok(app)
}

impl EditorApp {
    /// Shows the Help tab with the keyboard focus in the guide, so it
    /// scrolls with the keys.
    pub(super) fn show_guide(&mut self) {
        self.h.tabs.get().select(HELP_TAB);
        self.h.guide.get().focus();
    }

    /// Opens the ROM from the command line or the usual place; with none, the
    /// window stays in its empty state.
    fn start(&mut self, rom: Option<PathBuf>) {
        let path = rom.or_else(Rom::locate);
        if let Ok(zoom) = std::env::var("KSS_ZOOM").map(|z| z.parse::<i32>())
            && let Ok(zoom) = zoom
        {
            self.h.map.get().update_view(|v| v.set_zoom_centered(zoom));
        }
        match path {
            Some(path) => {
                match Editor::open(&path) {
                    Ok(editor) => {
                        *self.editor.borrow_mut() = Some(editor);
                        self.room_changed();
                        self.say(&format!("Opened {}  -  Save writes to a new \"(edited)\" file, never over this one", path.display()));
                    }
                    Err(e) => {
                        self.refresh_history_buttons();
                        self.refresh_status();
                        self.say(&format!("Could not open ROM: {e}"));
                    }
                }
            }
            None => {
                self.refresh_history_buttons();
                self.refresh_status();
                self.say("No ROM found. Use File > Open ROM... (or set KSS_ROM).");
            }
        }
    }
}
