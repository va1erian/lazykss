//! The settings window: theme and animations. It runs modally as its own
//! small app and hands the chosen [`Settings`] back when OK is pressed.

use xui::backend::PlatformSpec;
use xui::prelude::*;
use xui::widget::ComboBox;
use xui::{Dip, Theme};

use crate::settings::{Settings, ThemeChoice};

#[derive(Clone)]
enum SettingsMsg {
    Theme(usize),
    Animations(bool),
    Ok,
    Cancel,
}

struct SettingsApp {
    settings: Settings,
}

impl App for SettingsApp {
    type Msg = SettingsMsg;

    fn update(&mut self, msg: SettingsMsg, ui: &mut Ui<SettingsMsg>) {
        match msg {
            SettingsMsg::Theme(i) => self.settings.theme = ThemeChoice::from_index(i),
            SettingsMsg::Animations(on) => self.settings.animations = on,
            SettingsMsg::Ok => ui.close_with_result(self.settings),
            SettingsMsg::Cancel => ui.close(),
        }
    }
}

/// The toolkit theme a choice stands for; `None` keeps the platform's own.
fn theme_of(choice: ThemeChoice) -> Option<Theme> {
    Some(match choice {
        ThemeChoice::System => return None,
        ThemeChoice::Light => Theme::light(),
        ThemeChoice::Dark => Theme::dark(),
        ThemeChoice::Midnight => Theme::midnight(),
        ThemeChoice::Daylight => Theme::daylight(),
        ThemeChoice::Nightfall => Theme::nightfall(),
    })
}

/// Applies `settings` to the window `ui` drives.
pub fn apply<M: 'static>(ui: &Ui<M>, settings: &Settings) {
    if let Some(theme) = theme_of(settings.theme) {
        ui.set_theme(theme);
    }
    ui.set_animations(settings.animations);
}

/// Shows the settings window and returns the new settings if OK was pressed.
pub fn show<M: 'static>(ui: &Ui<M>, current: Settings) -> Option<Settings> {
    let spec = PlatformSpec::new("Settings").size(Dip(360.0), Dip(210.0));
    ui.open_modal::<SettingsApp, _, Settings>(spec, move |ui| build(ui, current))
}

fn build(ui: &mut Ui<SettingsMsg>, current: Settings) -> SettingsApp {
    let names: Vec<&str> = ThemeChoice::ALL.iter().map(|t| t.label()).collect();
    let index = current.theme.index();
    let root = column().padding(16).gap(10).children((
        label("Theme"),
        combo_box(&names)
            .then(move |c: ComboBox<SettingsMsg>| {
                c.select(index);
                c
            })
            .on_select(SettingsMsg::Theme),
        checkbox("Animations")
            .checked(current.animations)
            .on_toggle(SettingsMsg::Animations),
        label("Takes effect when you press OK, and is remembered."),
        row().gap(8).children((
            spacer(),
            button("OK").on_click(SettingsMsg::Ok),
            button("Cancel").on_click(SettingsMsg::Cancel),
        )),
    ));
    // A failure to mount leaves the window empty; Cancel/close still work.
    let _ = ui.root(root);
    SettingsApp { settings: current }
}
