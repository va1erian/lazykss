//! The room browser's rows: Spring Breeze levels with their rooms, then a
//! collapsed group of every other room.

use kss_core::room::SPRING_BREEZE_LEVELS;
use xui::widget::TreeRow;

use crate::editor::Editor;

/// What a row of the tree stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    /// A Spring Breeze level, by level index; also the index into the
    /// expansion flags.
    Level(usize),
    /// The "Other rooms" group.
    Other,
    Room(usize),
}

/// Number of expandable groups: the Spring Breeze levels plus "Other".
pub const GROUPS: usize = SPRING_BREEZE_LEVELS.end + 1;

pub fn group_index(node: Node) -> Option<usize> {
    match node {
        Node::Level(i) => Some(i),
        Node::Other => Some(SPRING_BREEZE_LEVELS.end),
        Node::Room(_) => None,
    }
}

fn room_label(editor: &Editor, room: usize) -> String {
    let dirty = if editor.is_dirty(room) { "*" } else { "" };
    match editor.loaded(room) {
        Some(lr) => format!("Room {room} \u{2014} {}x{}{dirty}", lr.room.width, lr.room.height),
        None => format!("Room {room}"),
    }
}

/// Rows in display order, with the node each stands for. Row `i` has the
/// tree node id `i`. `expanded` holds one flag per group.
pub fn rows(editor: &Editor, expanded: &[bool; GROUPS]) -> (Vec<TreeRow>, Vec<Node>) {
    let mut rows = Vec::new();
    let mut nodes = Vec::new();
    for level in SPRING_BREEZE_LEVELS {
        let info = &editor.tables().levels[level];
        let any_dirty = info.rooms.clone().any(|r| editor.is_dirty(r));
        let label = format!("{}{}", info.name, if any_dirty { " *" } else { "" });
        rows.push(TreeRow::new(label, 0).expandable(true).expanded(expanded[level]));
        nodes.push(Node::Level(level));
        for room in info.rooms.clone() {
            rows.push(TreeRow::new(room_label(editor, room), 1));
            nodes.push(Node::Room(room));
        }
    }
    let others = editor.spring_breeze_rooms().end..editor.room_count();
    if !others.is_empty() {
        let any_dirty = others.clone().any(|r| editor.is_dirty(r));
        let label = format!("Other rooms (unverified){}", if any_dirty { " *" } else { "" });
        rows.push(TreeRow::new(label, 0).expandable(true).expanded(expanded[SPRING_BREEZE_LEVELS.end]));
        nodes.push(Node::Other);
        for room in others {
            rows.push(TreeRow::new(room_label(editor, room), 1));
            nodes.push(Node::Room(room));
        }
    }
    (rows, nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kss_core::Rom;

    #[test]
    fn lists_levels_rooms_and_a_collapsed_group_of_the_rest() {
        let Some(path) = Rom::locate() else { return };
        let editor = Editor::open(&path).unwrap();
        let (rows, nodes) = rows(&editor, &[true, true, true, true, false]);
        assert_eq!(rows.len(), nodes.len());
        assert_eq!(rows[0].label, "Green Greens");
        assert!(rows[0].expanded && rows[0].expandable);
        let room1 = nodes.iter().position(|n| *n == Node::Room(1)).unwrap();
        assert_eq!(rows[room1].label, "Room 1 \u{2014} 100x8");
        let other = nodes.iter().position(|n| *n == Node::Other).unwrap();
        assert_eq!(rows[other].label, "Other rooms (unverified)");
        assert!(!rows[other].expanded);
        assert_eq!(nodes.iter().filter(|n| matches!(n, Node::Room(_))).count(), editor.room_count());
    }
}
