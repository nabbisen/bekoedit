//! RFC-050 slice 1: accessible names for Form Mode's fields that had none. Reached
//! from the fall-through arm of `form_history`'s lookup, in a file of their own to
//! keep `i18n.rs` within the ELOC limit.

pub(super) fn en(key: &str) -> &'static str {
    match key {
        "block.list_item" => "List item",
        "block.task_item" => "Task done",
        _ => "",
    }
}

pub(super) fn ja(key: &str) -> &'static str {
    match key {
        "block.list_item" => "リスト項目",
        "block.task_item" => "タスク完了",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_text_in_both_languages_and_they_differ() {
        for key in ["block.list_item", "block.task_item"] {
            assert!(!en(key).is_empty(), "{key} has no English");
            assert!(!ja(key).is_empty(), "{key} has no Japanese");
            assert_ne!(en(key), ja(key), "{key} reads the same in both languages");
        }
    }
}
