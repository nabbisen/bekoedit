//! The grid's cell keyboard (RFC-048 slice 5 §2.2): Tab and Shift+Tab move
//! between cells across rows, Enter commits and moves down, and Escape leaves
//! the cell for the table's first actions button.
//!
//! A move first commits the cell: a synthetic `change` event on the focused
//! input, the same flush `shortcuts.js` does before a shortcut (task 051),
//! then focus moves. The browser also fires `change` when focus leaves a
//! changed input, but the move does not depend on that.

use dioxus::prelude::*;

/// Where a key in a cell sends focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CellMove {
    /// Another cell, by row (0 is the header) and column.
    Cell(usize, usize),
    /// The table's "Add row" button; it does not add a row by itself.
    AddRow,
    /// The table's first column actions button.
    FirstActions,
    /// Commit and stay.
    Stay,
    /// Leave the key to the browser (Shift+Tab out of the first cell).
    Native,
}

/// Tab and Shift+Tab. `last_row` is the last data row's index, `last_col` the
/// last column's.
pub(super) fn tab_target(
    row: usize,
    col: usize,
    last_row: usize,
    last_col: usize,
    shift: bool,
) -> CellMove {
    if shift {
        if col > 0 {
            CellMove::Cell(row, col - 1)
        } else if row > 0 {
            CellMove::Cell(row - 1, last_col)
        } else {
            CellMove::Native
        }
    } else if col < last_col {
        CellMove::Cell(row, col + 1)
    } else if row < last_row {
        CellMove::Cell(row + 1, 0)
    } else {
        CellMove::AddRow
    }
}

/// Enter commits and moves down; on the last row it stays.
pub(super) fn enter_target(row: usize, col: usize, last_row: usize) -> CellMove {
    if row < last_row {
        CellMove::Cell(row + 1, col)
    } else {
        CellMove::Stay
    }
}

/// Commits the focused cell, then moves focus to `target`'s element.
pub(super) fn cell_move_script(field_id: &str, target: CellMove) -> String {
    let id = match target {
        CellMove::Cell(row, col) => Some(format!("{field_id}-{row}-{col}")),
        CellMove::AddRow => Some(format!("{field_id}-add-row")),
        CellMove::FirstActions => Some(format!("{field_id}-col-actions-0")),
        CellMove::Stay | CellMove::Native => None,
    };
    let focus = id.map_or(String::new(), |id| {
        format!("document.getElementById('{id}')?.focus();")
    });
    format!(
        "(() => {{ const el = document.activeElement; \
         if (el && el.tagName === 'INPUT') {{ \
         el.dispatchEvent(new Event('change', {{ bubbles: true }})); }} {focus} }})();"
    )
}

/// The `onkeydown` of a cell input.
pub(super) fn cell_keydown(
    event: &KeyboardEvent,
    field_id: &str,
    (row, col): (usize, usize),
    (last_row, last_col): (usize, usize),
) {
    let target = match event.key() {
        Key::Tab => tab_target(row, col, last_row, last_col, event.modifiers().shift()),
        Key::Enter => enter_target(row, col, last_row),
        Key::Escape => CellMove::FirstActions,
        _ => return,
    };
    if target == CellMove::Native {
        return;
    }
    event.prevent_default();
    document::eval(&cell_move_script(field_id, target));
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAST: (usize, usize) = (2, 1);

    #[test]
    fn tab_walks_the_row_then_the_next_row() {
        assert_eq!(
            tab_target(1, 0, LAST.0, LAST.1, false),
            CellMove::Cell(1, 1)
        );
        assert_eq!(
            tab_target(1, 1, LAST.0, LAST.1, false),
            CellMove::Cell(2, 0)
        );
        assert_eq!(
            tab_target(0, 1, LAST.0, LAST.1, false),
            CellMove::Cell(1, 0)
        );
    }

    #[test]
    fn tab_in_the_last_cell_goes_to_add_row_and_does_not_add_one() {
        assert_eq!(tab_target(2, 1, LAST.0, LAST.1, false), CellMove::AddRow);
    }

    #[test]
    fn shift_tab_walks_back_across_rows_and_leaves_the_first_cell_to_the_browser() {
        assert_eq!(tab_target(1, 1, LAST.0, LAST.1, true), CellMove::Cell(1, 0));
        assert_eq!(tab_target(2, 0, LAST.0, LAST.1, true), CellMove::Cell(1, 1));
        assert_eq!(tab_target(0, 0, LAST.0, LAST.1, true), CellMove::Native);
    }

    #[test]
    fn enter_moves_down_and_stays_on_the_last_row() {
        assert_eq!(enter_target(0, 1, 2), CellMove::Cell(1, 1));
        assert_eq!(enter_target(2, 1, 2), CellMove::Stay);
    }

    /// A move commits the cell before focus leaves it (the slice 5 mutation
    /// "Tab no longer commits before moving"): the `change` dispatch must
    /// come before the focus call. Mutation: delete the dispatch, or swap the
    /// two, and this fails.
    #[test]
    fn the_cell_is_committed_before_focus_moves() {
        let script = cell_move_script("fb-1-2", CellMove::Cell(2, 0));
        let commit = script
            .find("dispatchEvent(new Event('change'")
            .expect("a commit");
        let focus = script
            .find("getElementById('fb-1-2-2-0')")
            .expect("a focus");
        assert!(commit < focus, "{script}");
    }

    #[test]
    fn each_target_names_its_element() {
        assert!(cell_move_script("f", CellMove::AddRow).contains("'f-add-row'"));
        assert!(cell_move_script("f", CellMove::FirstActions).contains("'f-col-actions-0'"));
        assert!(!cell_move_script("f", CellMove::Stay).contains("getElementById"));
    }
}
