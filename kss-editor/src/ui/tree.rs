//! The room browser's rows: the named levels (Spring Breeze, The Great Cave
//! Offensive) with their rooms, then a collapsed group of every other room.

use kss_core::room::{GREAT_CAVE_OFFENSIVE, NAMED_LEVELS};
use xui::widget::TreeRow;

use crate::editor::Editor;

/// What a row of the tree stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    /// A named level, by its position in `NAMED_LEVELS`; also the index
    /// into the expansion flags.
    Level(usize),
    /// The "Other rooms" group.
    Other,
    Room(usize),
}

/// Number of expandable groups: the named levels plus "Other".
pub const GROUPS: usize = NAMED_LEVELS.len() + 1;

pub fn group_index(node: Node) -> Option<usize> {
    match node {
        Node::Level(i) => Some(i),
        Node::Other => Some(NAMED_LEVELS.len()),
        Node::Room(_) => None,
    }
}

/// The groups open when a ROM is opened: the Spring Breeze stages. The cave
/// (81 rooms) and the other rooms start collapsed.
pub fn initial_expansion() -> [bool; GROUPS] {
    let mut expanded = [false; GROUPS];
    for (i, &level) in NAMED_LEVELS.iter().enumerate() {
        expanded[i] = level != GREAT_CAVE_OFFENSIVE;
    }
    expanded
}

fn room_label(editor: &Editor, room: usize) -> String {
    let dirty = if editor.is_dirty(room) { "*" } else { "" };
    match editor.loaded(room) {
        Some(lr) => format!(
            "Room {room} \u{2014} {}x{}{dirty}",
            lr.room.width, lr.room.height
        ),
        None => format!("Room {room}"),
    }
}

/// Rows in display order, with the node each stands for. Row `i` has the
/// tree node id `i`. `expanded` holds one flag per group.
pub fn rows(editor: &Editor, expanded: &[bool; GROUPS]) -> (Vec<TreeRow>, Vec<Node>) {
    let mut rows = Vec::new();
    let mut nodes = Vec::new();
    let count = editor.room_count();
    for (group, &level) in NAMED_LEVELS.iter().enumerate() {
        let Some(info) = editor.tables().levels.get(level) else {
            continue;
        };
        let rooms = info.rooms.start.min(count)..info.rooms.end.min(count);
        let any_dirty = rooms.clone().any(|r| editor.is_dirty(r));
        let label = format!("{}{}", info.name, if any_dirty { " *" } else { "" });
        rows.push(
            TreeRow::new(label, 0)
                .expandable(true)
                .expanded(expanded[group]),
        );
        nodes.push(Node::Level(group));
        for room in rooms {
            rows.push(TreeRow::new(room_label(editor, room), 1));
            nodes.push(Node::Room(room));
        }
    }
    let named = editor.named_rooms();
    let others: Vec<usize> = (0..count).filter(|r| !named.contains(r)).collect();
    if !others.is_empty() {
        let any_dirty = others.iter().any(|&r| editor.is_dirty(r));
        let label = format!(
            "Other rooms (unverified){}",
            if any_dirty { " *" } else { "" }
        );
        rows.push(
            TreeRow::new(label, 0)
                .expandable(true)
                .expanded(expanded[NAMED_LEVELS.len()]),
        );
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
        let (rows, nodes) = rows(&editor, &initial_expansion());
        assert_eq!(rows.len(), nodes.len());
        assert_eq!(rows[0].label, "Green Greens");
        assert!(rows[0].expanded && rows[0].expandable);
        let room1 = nodes.iter().position(|n| *n == Node::Room(1)).unwrap();
        assert_eq!(rows[room1].label, "Room 1 \u{2014} 100x8");
        let cave = nodes.iter().position(|n| *n == Node::Level(4)).unwrap();
        assert_eq!(rows[cave].label, "The Great Cave Offensive");
        assert!(!rows[cave].expanded);
        assert_eq!(nodes[cave + 1], Node::Room(72));
        assert_eq!(nodes[cave + 81], Node::Room(152));
        let other = nodes.iter().position(|n| *n == Node::Other).unwrap();
        assert_eq!(rows[other].label, "Other rooms (unverified)");
        assert!(!rows[other].expanded);
        assert_eq!(nodes[other + 1], Node::Room(27));
        // Every room is listed exactly once.
        let mut listed: Vec<usize> = nodes
            .iter()
            .filter_map(|n| match n {
                Node::Room(r) => Some(*r),
                _ => None,
            })
            .collect();
        listed.sort();
        assert_eq!(listed, (0..editor.room_count()).collect::<Vec<_>>());
    }
}
