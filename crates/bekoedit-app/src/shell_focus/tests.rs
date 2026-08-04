use super::*;

/// A defect in a `document::eval` template — an unbalanced paren or brace —
/// throws a `SyntaxError` in the WebView and silently disables whatever it
/// was supposed to do. No Rust gate (fmt, clippy, unit tests over the
/// fragments they're built from) catches this: the defect lives in the
/// *assembly*, not in any tested fragment (RFC-042 slice 3 re-review C1/C2).
/// Rust cannot parse JavaScript, so this only checks delimiter balance — but
/// that is exactly what the slice-3 defect violated.
fn is_balanced(script: &str) -> bool {
    let mut parens = 0i32;
    let mut braces = 0i32;
    for c in script.chars() {
        match c {
            '(' => parens += 1,
            ')' => parens -= 1,
            '{' => braces += 1,
            '}' => braces -= 1,
            _ => {}
        }
        if parens < 0 || braces < 0 {
            return false;
        }
    }
    parens == 0 && braces == 0
}

#[test]
fn every_eval_script_has_balanced_parens_and_braces() {
    assert!(is_balanced(&focus_element_script(TRIGGER_APP_MENU)));
    assert!(is_balanced(&focus_tree_row_script(0)));
    for position in [
        FocusMove::First,
        FocusMove::Last,
        FocusMove::Next,
        FocusMove::Previous,
    ] {
        assert!(is_balanced(&focus_menu_item_script(
            MENU_APP_OVERFLOW,
            position
        )));
        assert!(is_balanced(&focus_tab_script(position)));
    }
}

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
