//! Keyboard shortcuts, as pure functions of the key and modifiers.

use xui::prelude::*;

use super::Msg;
use crate::editor::Tool;

/// Window-wide shortcuts (Ctrl combinations).
pub fn shortcut(key: Key, mods: Modifiers) -> Option<Msg> {
    if key == Key::F1 && mods == Modifiers::NONE {
        return Some(Msg::ShowGuide);
    }
    if !mods.ctrl || mods.alt {
        return None;
    }
    match (key, mods.shift) {
        (Key::Z, false) => Some(Msg::Undo),
        (Key::Z, true) | (Key::Y, _) => Some(Msg::Redo),
        (Key::S, false) => Some(Msg::Save),
        (Key::S, true) => Some(Msg::SaveAs),
        (Key::O, false) => Some(Msg::OpenRom),
        _ => None,
    }
}

/// What a key does while the map has the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKey {
    DeleteObject,
    /// Move the selected object by pixels.
    Nudge(i32, i32),
    Tool(Tool),
    Zoom(i32),
    Escape,
}

pub fn map_key(key: Key, mods: Modifiers) -> Option<MapKey> {
    if mods.ctrl || mods.alt {
        return None;
    }
    let step = if mods.shift { 8 } else { 1 };
    Some(match key {
        Key::DELETE | Key::BACK => MapKey::DeleteObject,
        Key::LEFT => MapKey::Nudge(-step, 0),
        Key::RIGHT => MapKey::Nudge(step, 0),
        Key::UP => MapKey::Nudge(0, -step),
        Key::DOWN => MapKey::Nudge(0, step),
        Key::V => MapKey::Tool(Tool::Select),
        Key::B => MapKey::Tool(Tool::Paint),
        Key::R => MapKey::Tool(Tool::Rect),
        Key::G => MapKey::Tool(Tool::Fill),
        Key::S => MapKey::Tool(Tool::Stamp),
        Key::O => MapKey::Tool(Tool::AddObject),
        Key::DIGIT1 => MapKey::Zoom(1),
        Key::DIGIT2 => MapKey::Zoom(2),
        Key::DIGIT3 => MapKey::Zoom(3),
        Key::DIGIT4 => MapKey::Zoom(4),
        Key::ESCAPE => MapKey::Escape,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Modifiers = Modifiers {
        ctrl: true,
        shift: false,
        alt: false,
        win: false,
    };

    #[test]
    fn ctrl_shortcuts() {
        assert!(matches!(shortcut(Key::Z, CTRL), Some(Msg::Undo)));
        assert!(matches!(shortcut(Key::Y, CTRL), Some(Msg::Redo)));
        assert!(matches!(
            shortcut(
                Key::Z,
                Modifiers {
                    shift: true,
                    ..CTRL
                }
            ),
            Some(Msg::Redo)
        ));
        assert!(matches!(
            shortcut(
                Key::S,
                Modifiers {
                    shift: true,
                    ..CTRL
                }
            ),
            Some(Msg::SaveAs)
        ));
        assert!(shortcut(Key::Z, Modifiers::NONE).is_none());
        assert!(matches!(
            shortcut(Key::F1, Modifiers::NONE),
            Some(Msg::ShowGuide)
        ));
    }

    #[test]
    fn map_keys() {
        assert_eq!(
            map_key(Key::LEFT, Modifiers::NONE),
            Some(MapKey::Nudge(-1, 0))
        );
        assert_eq!(
            map_key(
                Key::DOWN,
                Modifiers {
                    shift: true,
                    ..Modifiers::NONE
                }
            ),
            Some(MapKey::Nudge(0, 8))
        );
        assert_eq!(
            map_key(Key::B, Modifiers::NONE),
            Some(MapKey::Tool(Tool::Paint))
        );
        assert_eq!(map_key(Key::B, CTRL), None);
    }
}
