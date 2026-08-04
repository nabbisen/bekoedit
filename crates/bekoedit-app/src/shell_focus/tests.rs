use super::*;

#[test]
fn trigger_down_and_enter_and_space_open_to_first() {
    assert_eq!(trigger_key_intent(&Key::ArrowDown), Some(FocusMove::First));
    assert_eq!(trigger_key_intent(&Key::Enter), Some(FocusMove::First));
    assert_eq!(
        trigger_key_intent(&Key::Character(" ".to_string())),
        Some(FocusMove::First)
    );
}

#[test]
fn trigger_up_opens_to_last() {
    assert_eq!(trigger_key_intent(&Key::ArrowUp), Some(FocusMove::Last));
}

#[test]
fn trigger_ignores_unrelated_keys() {
    for key in [Key::Escape, Key::Tab, Key::Home, Key::End] {
        assert_eq!(trigger_key_intent(&key), None);
    }
}

#[test]
fn menu_item_down_up_home_end_map_to_the_expected_move() {
    assert_eq!(menu_item_key_intent(&Key::ArrowDown), Some(FocusMove::Next));
    assert_eq!(
        menu_item_key_intent(&Key::ArrowUp),
        Some(FocusMove::Previous)
    );
    assert_eq!(menu_item_key_intent(&Key::Home), Some(FocusMove::First));
    assert_eq!(menu_item_key_intent(&Key::End), Some(FocusMove::Last));
}

#[test]
fn menu_item_does_not_intercept_enter_or_space() {
    // Native button-click activation already handles these via the item's
    // own onclick; intercepting them here would risk double-activation.
    assert_eq!(menu_item_key_intent(&Key::Enter), None);
    assert_eq!(menu_item_key_intent(&Key::Character(" ".to_string())), None);
}

#[test]
fn tab_left_right_home_end_map_to_the_expected_move() {
    assert_eq!(tab_key_intent(&Key::ArrowRight), Some(FocusMove::Next));
    assert_eq!(tab_key_intent(&Key::ArrowLeft), Some(FocusMove::Previous));
    assert_eq!(tab_key_intent(&Key::Home), Some(FocusMove::First));
    assert_eq!(tab_key_intent(&Key::End), Some(FocusMove::Last));
}

#[test]
fn tab_does_not_intercept_enter_or_space() {
    // RFC-042 §7.3 requires manual activation: arrow keys move focus only.
    // Automatic activation on arrow would fire a protected command per
    // keystroke.
    assert_eq!(tab_key_intent(&Key::Enter), None);
    assert_eq!(tab_key_intent(&Key::Character(" ".to_string())), None);
}

#[test]
fn tab_ignores_up_and_down_unlike_the_tree_or_menus() {
    // Tabs move with Left/Right, not Up/Down (handoff §5.5) — distinct from
    // both the tree (slice 2, Up/Down) and menus (this slice, Up/Down).
    assert_eq!(tab_key_intent(&Key::ArrowUp), None);
    assert_eq!(tab_key_intent(&Key::ArrowDown), None);
}

#[test]
fn focus_move_expr_first_and_last_do_not_reference_current() {
    assert_eq!(focus_move_expr("items", FocusMove::First), "items[0]");
    assert_eq!(
        focus_move_expr("items", FocusMove::Last),
        "items[items.length - 1]"
    );
}

#[test]
fn focus_move_expr_next_and_previous_wrap_via_current() {
    assert_eq!(
        focus_move_expr("tabs", FocusMove::Next),
        "tabs[(current + 1) % tabs.length]"
    );
    assert_eq!(
        focus_move_expr("tabs", FocusMove::Previous),
        "tabs[(current - 1 + tabs.length) % tabs.length]"
    );
}
