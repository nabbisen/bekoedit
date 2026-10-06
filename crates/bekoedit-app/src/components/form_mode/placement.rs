//! Placement of an open table menu (RFC-048 slice 3's row menu, task 058;
//! reused by slice 4's column menu rather than copied).

/// Places a row's menu (task 058 §2.2) against its own trigger: below it,
/// or above when there is no room below, right-aligned to it and kept
/// inside the viewport. Re-run on every scroll, so the menu stays
/// attached to its row. Fixed positioning is what keeps the menu out of
/// `.table-block`'s overflow clip. The listener removes itself once the
/// menu is gone.
pub(super) fn place_menu_script(menu_id: &str, trigger_id: &str) -> String {
    let menu_id = crate::bridge::js_string_literal(menu_id);
    let trigger_id = crate::bridge::js_string_literal(trigger_id);
    format!(
        r#"(() => {{
            const menuId = {menu_id};
            const triggerId = {trigger_id};
            const place = () => {{
                const menu = document.getElementById(menuId);
                const trigger = document.getElementById(triggerId);
                if (!menu || !trigger) return false;
                const t = trigger.getBoundingClientRect();
                const m = menu.getBoundingClientRect();
                const below = t.bottom + 2;
                const top = below + m.height <= window.innerHeight
                    ? below
                    : Math.max(0, t.top - m.height - 2);
                const left = Math.max(0, Math.min(t.right - m.width, window.innerWidth - m.width));
                menu.style.top = top + "px";
                menu.style.left = left + "px";
                menu.style.visibility = "visible";
                return true;
            }};
            const onScroll = () => {{
                if (!place()) document.removeEventListener("scroll", onScroll, true);
            }};
            // An open menu closes when the window is resized (slice 5 §2.3):
            // its trigger's own click toggles it shut.
            const onResize = () => {{
                window.removeEventListener("resize", onResize);
                document.removeEventListener("scroll", onScroll, true);
                const trigger = document.getElementById(triggerId);
                if (document.getElementById(menuId) && trigger) trigger.click();
            }};
            place();
            document.addEventListener("scroll", onScroll, true);
            window.addEventListener("resize", onResize);
        }})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Task 058 §2.2: the placement script must stay syntactically whole
    /// (an unbalanced brace is a silent WebView SyntaxError), and must
    /// name both the flip-upward fallback and the scroll re-placement.
    #[test]
    fn the_placement_script_is_balanced_and_handles_flip_and_scroll() {
        let script = place_menu_script("fb-1-2-row-menu-2", "fb-1-2-row-actions-2");
        assert!(is_balanced(&script), "{script}");
        assert!(script.contains("t.top - m.height"), "flip-upward fallback");
        assert!(
            script.contains("addEventListener(\"scroll\""),
            "re-placed on scroll"
        );
    }

    /// RFC-048 slice 5 §2.3: the menu is hidden by CSS until it is placed, so
    /// it never shows at its static position for a frame, and it closes when
    /// the window is resized. Mutation: drop the `visibility` line, and the
    /// menu stays hidden for good; drop the resize listener, and it stays
    /// where the window used to put it.
    #[test]
    fn the_menu_is_shown_only_once_placed_and_closes_on_resize() {
        let script = place_menu_script("m", "t");
        let placed = script.find("menu.style.left").expect("the placement");
        let shown = script
            .find("menu.style.visibility = \"visible\"")
            .expect("made visible");
        assert!(placed < shown, "visible only after it is positioned");
        assert!(
            script.contains("addEventListener(\"resize\""),
            "closes on resize"
        );
        assert!(is_balanced(&script), "{script}");
    }
}
