//! GUI internationalization (project requirement: the GUI is i18n-ready).
//!
//! Core crates emit stable label keys (`save.*`, `island.*`); the GUI
//! resolves them here. MVP ships English and Japanese tables; adding a
//! language is adding one match arm per key.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    En,
    Ja,
}

impl Lang {
    #[allow(dead_code)]
    pub fn toggle(self) -> Self {
        match self {
            Lang::En => Lang::Ja,
            Lang::Ja => Lang::En,
        }
    }
}

/// Resolves a label key for `lang`. Unknown keys return an empty string;
/// inventory tests keep all user-visible call sites translated.
pub fn tr(lang: Lang, key: &str) -> &'static str {
    match lang {
        Lang::En => tr_en(key),
        Lang::Ja => tr_ja(key),
    }
}

fn tr_en(key: &str) -> &'static str {
    match key {
        "app.title" => "bekoedit",
        "app.tagline" => "A source-preserving Markdown editor",
        "start.open_folder" => "Open Folder",
        "start.new_file" => "New",
        "start.tagline" => "A source-preserving Markdown editor",
        "start.open" => "Open Folder as Workspace",
        "start.path_placeholder" => "Path to a folder…",
        "start.recents" => "Recent workspaces",
        "start.no_recents" => "No recent workspaces yet.",
        "explorer.new_file" => "New file",
        "explorer.name_placeholder" => "name",
        "explorer.rename" => "Rename",
        "explorer.delete" => "Delete",
        "explorer.empty" => "No Markdown files in this workspace yet.",
        "mode.text" => "Text",
        "mode.form" => "Form",
        "mode.preview" => "Preview",
        "editor.save" => "Save",
        "editor.save_as" => "Save As",
        "editor.save_as_title" => "Save Markdown file",
        "editor.untitled" => "Untitled",
        "editor.no_document" => "Select a Markdown file to start editing.",
        "editor.loading" => "Loading editor…",
        "editor.unavailable" => "Editor unavailable",
        "editor.retry" => "Retry",
        "save.clean" => "No changes",
        "save.dirty" => "Unsaved changes",
        "save.scheduled" => "Autosave pending…",
        "save.saving" => "Saving…",
        "save.saved" => "All changes saved",
        "save.failed" => "Save failed — your text is kept in memory",
        "save.external_change" => "File changed outside bekoedit",
        "save.conflict" => "File changed on disk",
        "conflict.title" => "This file was modified outside bekoedit.",
        "conflict.keep_mine" => "Keep my version",
        "conflict.reload" => "Reload from disk",
        "conflict.save_copy" => "Save my version as a copy",
        "conflict.deleted" => "This file was deleted on disk.",
        "island.front_matter" => "Front matter",
        "island.html_block" => "HTML block",
        "island.complex_table" => "Table (raw Markdown)",
        "island.math_block" => "Math block",
        "island.directive" => "Directive",
        "island.complex_list" => "Nested list (raw Markdown)",
        "island.complex_blockquote" => "Blockquote (raw Markdown)",
        "island.unknown_extension" => "Unrecognized Markdown",
        "island.malformed_region" => "Malformed region",
        "island.footnote" => "Footnote",
        "island.hint" => "Edited as raw Markdown to preserve your source exactly.",
        "block.delete" => "Delete block",
        "block.heading_level" => "Level",
        "block.language" => "Language",
        "table.add_row" => "Add row",
        "table.empty" => "This table has no columns.",
        "error.generic" => "Something went wrong",
        "lang.switch" => "日本語",
        "mode.split" => "Split",
        "mode.close_split" => "Close split",
        "outline.label" => "Document outline",
        "outline.title" => "Outline",
        "outline.toggle" => "Toggle outline panel",
        "outline.empty" => "No headings in this document.",
        "outline.move_up" => "Move section up",
        "outline.move_down" => "Move section down",
        "backlinks.title" => "Linked from",
        "backlinks.empty" => "No backlinks to this document.",
        "backlinks.label" => "Documents linking here",
        "backlinks.count_suffix" => " links",
        "history.title" => "History",
        "history.empty" => "No history entries.",
        "history.label" => "Document history",
        "history.restore" => "Restore",
        "history.restored" => "History restored",
        "templates.label" => "Templates",
        "templates.empty" => "No templates available.",
        "templates.blank" => "Blank document",
        "search.label" => "Search",
        "search.placeholder" => "Search workspace…",
        "search.submit" => "Search",
        "search.close" => "Close search",
        "search.empty" => "No matches.",
        "settings.title" => "Settings",
        "settings.close" => "Close",
        "settings.save" => "Save settings",
        "settings.general" => "General",
        "settings.editor" => "Editor",
        "settings.language" => "Language",
        "settings.default_mode" => "Default editing mode",
        "settings.reopen" => "Reopen last workspace on start",
        "settings.autosave_ms" => "Autosave delay",
        "settings.prefer_trash" => "Move deleted files to trash",
        "recovery.title" => "Recover unsaved changes",
        "recovery.description" => {
            "bekoedit found unsaved recovery snapshots from a previous session."
        }
        "recovery.restore" => "Restore",
        "recovery.discard" => "Discard",
        "recovery.skip_all" => "Discard all",
        "recovery.restored" => "Recovery restored",
        "toast.dismiss" => "Dismiss notification",
        "editor.toolbar_label" => "Editor toolbar",
        "editor.mode_label" => "Editing mode",
        "explorer.region_label" => "Workspace explorer",
        "explorer.tree_label" => "Files",
        "explorer.toggle" => "Toggle sidebar",
        "status.islands_hint" => "Raw Markdown Islands (preserved regions)",
        "status.diag_hint" => "Parse diagnostics",
        "status.words" => "words",
        "status.chars" => "characters",
        _ => "",
    }
}

fn tr_ja(key: &str) -> &'static str {
    match key {
        "app.title" => "bekoedit",
        "app.tagline" => "ソースを保全するMarkdownエディタ",
        "start.open_folder" => "フォルダを開く",
        "start.new_file" => "新規",
        "start.tagline" => "ソースを保全するMarkdownエディタ",
        "start.open" => "フォルダをワークスペースとして開く",
        "start.path_placeholder" => "フォルダのパス…",
        "start.recents" => "最近のワークスペース",
        "start.no_recents" => "最近のワークスペースはまだありません。",
        "explorer.new_file" => "新規ファイル",
        "explorer.name_placeholder" => "名前",
        "explorer.rename" => "名前を変更",
        "explorer.delete" => "削除",
        "explorer.empty" => "このワークスペースにはまだMarkdownファイルがありません。",
        "mode.text" => "テキスト",
        "mode.form" => "フォーム",
        "mode.preview" => "プレビュー",
        "editor.save" => "保存",
        "editor.save_as" => "名前を付けて保存",
        "editor.save_as_title" => "Markdownファイルを保存",
        "editor.untitled" => "無題",
        "editor.no_document" => "編集するMarkdownファイルを選択してください。",
        "editor.loading" => "エディタを読み込んでいます…",
        "editor.unavailable" => "エディタを利用できません",
        "editor.retry" => "再試行",
        "save.clean" => "変更なし",
        "save.dirty" => "未保存の変更があります",
        "save.scheduled" => "自動保存待機中…",
        "save.saving" => "保存中…",
        "save.saved" => "すべての変更を保存しました",
        "save.failed" => "保存に失敗しました — テキストはメモリに保持されています",
        "save.external_change" => "ファイルがbekoeditの外部で変更されました",
        "save.conflict" => "ファイルがディスク上で変更されました",
        "conflict.title" => "このファイルはbekoeditの外部で変更されました。",
        "conflict.keep_mine" => "自分の変更を保持",
        "conflict.reload" => "ディスクから再読み込み",
        "conflict.save_copy" => "コピーとして保存",
        "conflict.deleted" => "このファイルはディスク上で削除されました。",
        "island.front_matter" => "フロントマター",
        "island.html_block" => "HTMLブロック",
        "island.complex_table" => "表（生のMarkdown）",
        "island.math_block" => "数式ブロック",
        "island.directive" => "ディレクティブ",
        "island.complex_list" => "ネストされたリスト（生のMarkdown）",
        "island.complex_blockquote" => "引用（生のMarkdown）",
        "island.unknown_extension" => "未対応のMarkdown",
        "island.malformed_region" => "不正な領域",
        "island.footnote" => "脚注",
        "island.hint" => "ソースを正確に保全するため、生のMarkdownとして編集します。",
        "block.delete" => "ブロックを削除",
        "block.heading_level" => "レベル",
        "block.language" => "言語",
        "table.add_row" => "行を追加",
        "table.empty" => "この表には列がありません。",
        "error.generic" => "問題が発生しました",
        "lang.switch" => "English",
        "mode.split" => "分割",
        "mode.close_split" => "分割表示を閉じる",
        "outline.label" => "ドキュメントアウトライン",
        "outline.title" => "アウトライン",
        "outline.toggle" => "アウトラインパネルの切り替え",
        "outline.empty" => "このドキュメントに見出しがありません。",
        "outline.move_up" => "セクションを上へ移動",
        "outline.move_down" => "セクションを下へ移動",
        "backlinks.title" => "リンク元",
        "backlinks.empty" => "このドキュメントへのリンクはありません。",
        "backlinks.label" => "ここへリンクしているドキュメント",
        "backlinks.count_suffix" => "件のリンク",
        "history.title" => "履歴",
        "history.empty" => "履歴はありません。",
        "history.label" => "ドキュメント履歴",
        "history.restore" => "復元",
        "history.restored" => "履歴を復元しました",
        "templates.label" => "テンプレート",
        "templates.empty" => "利用できるテンプレートはありません。",
        "templates.blank" => "空のドキュメント",
        "search.label" => "検索",
        "search.placeholder" => "ワークスペースを検索…",
        "search.submit" => "検索",
        "search.close" => "検索を閉じる",
        "search.empty" => "一致する項目はありません。",
        "settings.title" => "設定",
        "settings.close" => "閉じる",
        "settings.save" => "設定を保存",
        "settings.general" => "一般",
        "settings.editor" => "エディタ",
        "settings.language" => "言語",
        "settings.default_mode" => "デフォルト編集モード",
        "settings.reopen" => "起動時に最後のワークスペースを再開",
        "settings.autosave_ms" => "自動保存の遅延",
        "settings.prefer_trash" => "削除したファイルをゴミ箱に移動",
        "recovery.title" => "未保存の変更を復元",
        "recovery.description" => {
            "前回のセッションで保存されなかった復元スナップショットが見つかりました。"
        }
        "recovery.restore" => "復元",
        "recovery.discard" => "破棄",
        "recovery.skip_all" => "すべて破棄",
        "recovery.restored" => "復元しました",
        "toast.dismiss" => "通知を閉じる",
        "editor.toolbar_label" => "エディタツールバー",
        "editor.mode_label" => "編集モード",
        "explorer.region_label" => "ワークスペースエクスプローラ",
        "explorer.tree_label" => "ファイル",
        "explorer.toggle" => "サイドバーを切り替え",
        "status.islands_hint" => "Raw Markdownアイランド（保全された領域）",
        "status.diag_hint" => "パース警告",
        "status.words" => "単語",
        "status.chars" => "文字",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_label_keys_are_translated_in_both_languages() {
        let keys = [
            "save.clean",
            "save.conflict",
            "island.front_matter",
            "island.malformed_region",
            "conflict.keep_mine",
            "mode.form",
            "editor.loading",
            "editor.unavailable",
            "editor.retry",
        ];
        for key in keys {
            assert!(!tr(Lang::En, key).is_empty(), "missing en: {key}");
            assert!(!tr(Lang::Ja, key).is_empty(), "missing ja: {key}");
        }
    }

    #[test]
    fn unknown_keys_fall_back_safely() {
        assert_eq!(tr(Lang::En, "nope.nope"), "");
    }
}
