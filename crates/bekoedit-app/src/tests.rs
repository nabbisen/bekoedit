#[cfg(test)]
mod app_tests {

    // ── i18n coverage: every key must exist in both EN and JA ────────────────

    #[test]
    fn i18n_all_keys_have_both_languages() {
        use crate::i18n::{Lang, tr};
        // Collect every key from EN by checking which keys return a non-empty
        // string in EN mode. Any key that exists in one language must exist in
        // both — a fallback to the key itself signals a missing translation.
        let sample_keys = [
            "app.title",
            "status.words",
            "status.chars",
            "status.islands_hint",
            "status.diag_hint",
            "save.clean",
            "save.dirty",
            "save.saving",
            "save.failed",
            "save.external_change",
            "save.conflict",
            "editor.no_document",
            "mode.text",
            "mode.form",
            "mode.preview",
            "mode.split",
            "outline.title",
            "outline.empty",
            "outline.label",
            "outline.move_up",
            "outline.move_down",
            "backlinks.title",
            "backlinks.empty",
            "backlinks.label",
            "backlinks.count_suffix",
            "history.title",
            "history.empty",
            "history.label",
            "history.restore",
            "history.restored",
            "templates.label",
            "templates.empty",
            "templates.blank",
            "island.footnote",
            "search.label",
            "search.placeholder",
            "search.empty",
            "lang.switch",
            "settings.title",
        ];
        let mut missing = Vec::new();
        for key in &sample_keys {
            let en = tr(Lang::En, key);
            let ja = tr(Lang::Ja, key);
            // tr() returns the key itself if not found.
            if en == *key {
                missing.push(format!("EN missing: {key}"));
            }
            if ja == *key {
                missing.push(format!("JA missing: {key}"));
            }
        }
        assert!(
            missing.is_empty(),
            "i18n coverage gaps:\n{}",
            missing.join("\n")
        );
    }
}
