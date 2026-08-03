//! Pure keyboard-navigation logic for the workspace tree (RFC-042 §7.1).
//!
//! No DOM, no Dioxus signal — a function over a row view, an active index,
//! and a key. This is deliberate: the reducer is the part of a roving-
//! tabindex tree that is easiest to get subtly wrong, and it is the part
//! that needs no display to test.

use std::path::{Path, PathBuf};

/// The navigation-relevant shape of one visible row. Order in the slice
/// passed to [`navigate`] must match render order (i.e. `visible_rows()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavRow {
    pub is_dir: bool,
    pub is_expanded: bool,
    pub depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavOutcome {
    /// Move the active row to this index; the caller focuses it.
    Move(usize),
    /// Expand the directory at this index; the caller re-derives rows and
    /// keeps focus on the same (now-expanded) row.
    Expand(usize),
    /// Collapse the directory at this index; same focus contract as above.
    Collapse(usize),
    /// No navigable outcome — already at a boundary, or the key does
    /// nothing for this row kind.
    None,
}

/// Navigate the tree per RFC-042 §7.1's key table. Never panics, including
/// on an empty `rows` or an out-of-range `active`.
pub fn navigate(rows: &[NavRow], active: usize, key: NavKey) -> NavOutcome {
    if rows.is_empty() {
        return NavOutcome::None;
    }
    let active = active.min(rows.len() - 1);
    match key {
        NavKey::Up => move_to(active.checked_sub(1)),
        NavKey::Down => {
            let next = active + 1;
            move_to((next < rows.len()).then_some(next))
        }
        NavKey::Home => move_to((active != 0).then_some(0)),
        NavKey::End => {
            let last = rows.len() - 1;
            move_to((active != last).then_some(last))
        }
        NavKey::Right => {
            let row = rows[active];
            if !row.is_dir {
                NavOutcome::None
            } else if !row.is_expanded {
                NavOutcome::Expand(active)
            } else {
                first_child(rows, active)
            }
        }
        NavKey::Left => {
            let row = rows[active];
            if row.is_dir && row.is_expanded {
                NavOutcome::Collapse(active)
            } else {
                parent_of(rows, active)
            }
        }
    }
}

fn move_to(index: Option<usize>) -> NavOutcome {
    index.map_or(NavOutcome::None, NavOutcome::Move)
}

fn first_child(rows: &[NavRow], active: usize) -> NavOutcome {
    let child_depth = rows[active].depth + 1;
    let next = active + 1;
    if next < rows.len() && rows[next].depth == child_depth {
        NavOutcome::Move(next)
    } else {
        NavOutcome::None
    }
}

/// The nearest preceding row of strictly lower depth — the parent, even
/// across a multi-level depth gap (e.g. a deeply nested selection collapsing
/// straight back to a shallow ancestor).
fn parent_of(rows: &[NavRow], active: usize) -> NavOutcome {
    let depth = rows[active].depth;
    rows[..active]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, row)| row.depth < depth)
        .map_or(NavOutcome::None, |(index, _)| NavOutcome::Move(index))
}

/// Resolves a tracked active path to its current row index (RFC-042 §7.2):
/// exact match first, then the nearest surviving ancestor, then the first
/// row. `None` only when `paths` itself is empty — there is no row to be
/// active.
pub fn resolve_active_row(paths: &[PathBuf], active_path: &Path) -> Option<usize> {
    if paths.is_empty() {
        return None;
    }
    if let Some(position) = paths.iter().position(|path| path == active_path) {
        return Some(position);
    }
    active_path
        .ancestors()
        .skip(1)
        .find_map(|ancestor| paths.iter().position(|path| path == ancestor))
        .or(Some(0))
}

#[cfg(test)]
mod tests;
