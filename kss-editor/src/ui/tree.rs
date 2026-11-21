//! The room browser's rows: each sub-game, its levels and their rooms, then
//! a collapsed group of the rooms no sub-game plays (cut-scenes, dummies).

use std::collections::BTreeSet;
use std::ops::Range;

use kss_core::SubGame;
use xui::widget::TreeRow;

use crate::editor::Editor;

/// What a row of the tree stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Node {
    SubGame(SubGame),
    /// A level, by its index in the level table.
    Level(usize),
    /// The "Other rooms" group.
    Other,
    Room(usize),
}

/// The groups that are open. A node not in the set is collapsed.
pub type Expansion = BTreeSet<Node>;

/// Whether a node is a group, which can be opened and closed.
pub fn is_group(node: Node) -> bool {
    !matches!(node, Node::Room(_))
}

/// The groups open when a ROM is opened: Spring Breeze and its stages.
/// Every other sub-game, and the other rooms, start collapsed.
pub fn initial_expansion() -> Expansion {
    std::iter::once(Node::SubGame(SubGame::SpringBreeze))
        .chain(SubGame::SpringBreeze.levels().map(Node::Level))
        .collect()
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

/// The tree under construction: rows and the node each stands for.
struct Builder<'a> {
    editor: &'a Editor,
    expanded: &'a Expansion,
    rows: Vec<TreeRow>,
    nodes: Vec<Node>,
}

impl Builder<'_> {
    fn group(&mut self, node: Node, name: &str, depth: u16, rooms: &[usize]) {
        let dirty = rooms.iter().any(|&r| self.editor.is_dirty(r));
        let label = format!("{name}{}", if dirty { " *" } else { "" });
        self.rows.push(
            TreeRow::new(label, depth)
                .expandable(true)
                .expanded(self.expanded.contains(&node)),
        );
        self.nodes.push(node);
    }

    fn rooms(&mut self, rooms: &[usize], depth: u16) {
        for &room in rooms {
            self.rows
                .push(TreeRow::new(room_label(self.editor, room), depth));
            self.nodes.push(Node::Room(room));
        }
    }

    /// A sub-game, with a row per level; a sub-game of one level lists
    /// its rooms directly.
    fn sub_game(&mut self, game: SubGame) {
        let levels: Vec<(usize, String, Vec<usize>)> = game
            .levels()
            .filter_map(|i| self.editor.tables().levels.get(i))
            .map(|l| (l.index, l.name.clone(), self.clipped(l.rooms.clone())))
            .collect();
        let all: Vec<usize> = levels.iter().flat_map(|l| l.2.iter().copied()).collect();
        self.group(Node::SubGame(game), game.name(), 0, &all);
        if let [_] = levels.as_slice() {
            self.rooms(&all, 1);
            return;
        }
        for (index, name, rooms) in &levels {
            self.group(Node::Level(*index), name, 1, rooms);
            self.rooms(rooms, 2);
        }
    }

    fn clipped(&self, rooms: Range<usize>) -> Vec<usize> {
        let count = self.editor.room_count();
        (rooms.start.min(count)..rooms.end.min(count)).collect()
    }
}

/// Rows in display order, with the node each stands for. Row `i` has the
/// tree node id `i`.
pub fn rows(editor: &Editor, expanded: &Expansion) -> (Vec<TreeRow>, Vec<Node>) {
    let mut tree = Builder {
        editor,
        expanded,
        rows: Vec::new(),
        nodes: Vec::new(),
    };
    for game in SubGame::ALL {
        tree.sub_game(game);
    }
    let listed = editor.listed_rooms();
    let others: Vec<usize> = (0..editor.room_count())
        .filter(|r| !listed.contains(r))
        .collect();
    if !others.is_empty() {
        tree.group(Node::Other, "Other rooms (cut-scenes, unused)", 0, &others);
        tree.rooms(&others, 1);
    }
    (tree.rows, tree.nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kss_core::Rom;

    fn position(nodes: &[Node], node: Node) -> usize {
        nodes.iter().position(|n| *n == node).unwrap()
    }

    #[test]
    fn lists_sub_games_levels_rooms_and_a_collapsed_group_of_the_rest() {
        let Some(path) = Rom::locate() else { return };
        let editor = Editor::open(&path).unwrap();
        let (rows, nodes) = rows(&editor, &initial_expansion());
        assert_eq!(rows.len(), nodes.len());
        assert_eq!(rows[0].label, "Spring Breeze");
        assert!(rows[0].expanded && rows[0].expandable);
        assert_eq!((rows[1].label.as_str(), rows[1].depth), ("Green Greens", 1));
        assert!(rows[1].expanded);
        let room1 = position(&nodes, Node::Room(1));
        assert_eq!(rows[room1].label, "Room 1 \u{2014} 100x8");
        assert_eq!(rows[room1].depth, 2);
        let dyna = position(&nodes, Node::SubGame(SubGame::DynaBlade));
        assert_eq!(rows[dyna].label, "Dyna Blade");
        assert!(!rows[dyna].expanded);
        assert_eq!(nodes[dyna + 1], Node::Level(4));
        assert_eq!(rows[dyna + 1].label, "Peanut Plains");
        assert_eq!(nodes[dyna + 2], Node::Room(27));
        let cave = position(&nodes, Node::Level(15));
        assert_eq!(rows[cave].label, "The Great Cave Offensive");
        assert_eq!(nodes[cave + 1], Node::Room(72));
        assert_eq!(nodes[cave + 81], Node::Room(152));
        // The Arena is one level: its rooms hang off the sub-game.
        let arena = position(&nodes, Node::SubGame(SubGame::TheArena));
        assert_eq!(
            (nodes[arena + 1], rows[arena + 1].depth),
            (Node::Room(262), 1)
        );
        let other = position(&nodes, Node::Other);
        assert_eq!(rows[other].label, "Other rooms (cut-scenes, unused)");
        assert!(!rows[other].expanded);
        assert_eq!(nodes[other + 1], Node::Room(284));
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
