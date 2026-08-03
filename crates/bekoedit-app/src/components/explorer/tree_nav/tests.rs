use super::*;

fn row(is_dir: bool, is_expanded: bool, depth: u32) -> NavRow {
    NavRow {
        is_dir,
        is_expanded,
        depth,
    }
}

fn file(depth: u32) -> NavRow {
    row(false, false, depth)
}

fn dir(is_expanded: bool, depth: u32) -> NavRow {
    row(true, is_expanded, depth)
}

#[test]
fn up_and_down_move_one_row_and_clamp_at_both_ends() {
    let rows = [file(0), file(0), file(0)];

    assert_eq!(navigate(&rows, 0, NavKey::Up), NavOutcome::None);
    assert_eq!(navigate(&rows, 1, NavKey::Up), NavOutcome::Move(0));
    assert_eq!(navigate(&rows, 1, NavKey::Down), NavOutcome::Move(2));
    assert_eq!(navigate(&rows, 2, NavKey::Down), NavOutcome::None);
}

#[test]
fn right_expands_collapsed_moves_into_expanded_none_on_file() {
    let rows = [dir(false, 0), dir(true, 0), file(1), file(0)];

    // Collapsed directory: expand in place.
    assert_eq!(navigate(&rows, 0, NavKey::Right), NavOutcome::Expand(0));
    // Expanded directory with a child immediately after: move into it.
    assert_eq!(navigate(&rows, 1, NavKey::Right), NavOutcome::Move(2));
    // File: no outcome.
    assert_eq!(navigate(&rows, 3, NavKey::Right), NavOutcome::None);
}

#[test]
fn right_on_expanded_directory_with_no_children_is_none() {
    // Expanded but immediately followed by a sibling at the same depth
    // (e.g. an empty directory) — no first child to move into.
    let rows = [dir(true, 0), dir(false, 0)];
    assert_eq!(navigate(&rows, 0, NavKey::Right), NavOutcome::None);
}

#[test]
fn left_collapses_expanded_directory() {
    let rows = [dir(true, 0)];
    assert_eq!(navigate(&rows, 0, NavKey::Left), NavOutcome::Collapse(0));
}

#[test]
fn left_on_collapsed_directory_or_file_moves_to_parent_across_a_depth_gap() {
    // dir(0) -> dir(1, expanded) -> dir(2, expanded) -> file(3): a plain
    // single-level parent lookup from the deepest row.
    let rows = [dir(true, 0), dir(true, 1), dir(true, 2), file(3)];
    assert_eq!(navigate(&rows, 3, NavKey::Left), NavOutcome::Move(2));

    // A/                     (row 0, depth 0, expanded)
    //   x/                   (row 1, depth 1, expanded)
    //     y.md                (row 2, depth 2)
    //   z.md                 (row 3, depth 1) <- back out of x/, sibling of x
    // From row 3, Left must skip past rows 1 and 2 (neither is an ancestor
    // of row 3) and land on row 0 — the nearest *lower*-depth row, not
    // merely the nearest preceding one.
    let rows = [dir(true, 0), dir(true, 1), file(2), file(1)];
    assert_eq!(navigate(&rows, 3, NavKey::Left), NavOutcome::Move(0));
}

#[test]
fn left_at_depth_zero_is_none() {
    let rows = [file(0)];
    assert_eq!(navigate(&rows, 0, NavKey::Left), NavOutcome::None);

    let rows = [dir(false, 0), file(0)];
    assert_eq!(navigate(&rows, 1, NavKey::Left), NavOutcome::None);
}

#[test]
fn home_and_end_reach_the_first_and_last_rows() {
    let rows = [file(0), file(0), file(0), file(0)];

    assert_eq!(navigate(&rows, 2, NavKey::Home), NavOutcome::Move(0));
    assert_eq!(navigate(&rows, 0, NavKey::Home), NavOutcome::None);
    assert_eq!(navigate(&rows, 1, NavKey::End), NavOutcome::Move(3));
    assert_eq!(navigate(&rows, 3, NavKey::End), NavOutcome::None);
}

#[test]
fn non_openable_rows_are_traversed_not_skipped() {
    // tree_nav has no concept of "openable" at all — Up/Down step through
    // every row uniformly, regardless of what a caller will later decide is
    // activatable. This is what makes that true: a plain file row is not
    // treated any differently from any other row by index-based movement.
    let rows = [dir(false, 0), file(0), file(0)];

    assert_eq!(navigate(&rows, 0, NavKey::Down), NavOutcome::Move(1));
    assert_eq!(navigate(&rows, 1, NavKey::Down), NavOutcome::Move(2));
}

#[test]
fn navigation_over_an_empty_row_list_returns_none_and_never_panics() {
    let rows: [NavRow; 0] = [];
    for key in [
        NavKey::Up,
        NavKey::Down,
        NavKey::Left,
        NavKey::Right,
        NavKey::Home,
        NavKey::End,
    ] {
        assert_eq!(navigate(&rows, 0, key), NavOutcome::None);
        // An out-of-range active index must not panic either.
        assert_eq!(navigate(&rows, 42, key), NavOutcome::None);
    }
}

#[test]
fn out_of_range_active_index_is_clamped_not_panicking() {
    let rows = [file(0), file(0)];
    // active = 9 clamps to the last row (index 1); Down there is a no-op.
    assert_eq!(navigate(&rows, 9, NavKey::Down), NavOutcome::None);
    assert_eq!(navigate(&rows, 9, NavKey::Up), NavOutcome::Move(0));
}

#[test]
fn active_path_recovery_prefers_exact_match() {
    let paths = [PathBuf::from("/w/a"), PathBuf::from("/w/b")];
    assert_eq!(resolve_active_row(&paths, Path::new("/w/b")), Some(1));
}

#[test]
fn active_path_recovery_falls_back_to_nearest_surviving_ancestor() {
    // /w/dir/child.md is gone (e.g. renamed or the directory collapsed and
    // was rescanned) but /w/dir itself is still a visible row.
    let paths = [PathBuf::from("/w/dir"), PathBuf::from("/w/other")];
    assert_eq!(
        resolve_active_row(&paths, Path::new("/w/dir/nested/child.md")),
        Some(0)
    );
}

#[test]
fn active_path_recovery_falls_back_to_the_first_row_when_no_ancestor_survives() {
    let paths = [PathBuf::from("/w/only")];
    assert_eq!(
        resolve_active_row(&paths, Path::new("/w/gone/deleted.md")),
        Some(0)
    );
}

#[test]
fn active_path_recovery_over_no_rows_is_none() {
    let paths: [PathBuf; 0] = [];
    assert_eq!(resolve_active_row(&paths, Path::new("/w/anything")), None);
}
