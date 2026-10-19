//! RFC-049 slice 2: the texts of Form Mode's Undo and Redo. Reached from the
//! fall-through arm of `tr_en` / `tr_ja`, in a file of their own to keep
//! `i18n.rs` within the ELOC limit.

pub(super) fn en(key: &str) -> &'static str {
    match key {
        "form.history.stale" => {
            "Nothing was undone or redone: the document changed since that edit."
        }
        "form.history.label" => "Undo and redo",
        "form.undo" => "Undo",
        "form.redo" => "Redo",
        _ => "",
    }
}

pub(super) fn ja(key: &str) -> &'static str {
    match key {
        "form.history.stale" => {
            "元に戻す・やり直しは行われませんでした: その編集のあとにドキュメントが変更されました。"
        }
        "form.history.label" => "元に戻す・やり直し",
        "form.undo" => "元に戻す",
        "form.redo" => "やり直し",
        _ => "",
    }
}
