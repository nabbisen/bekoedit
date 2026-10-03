#[cfg(test)]
mod rfc_042;
#[cfg(test)]
mod task_051_shortcut_flush;

#[cfg(test)]
mod app_tests {
    /// A floor for `all_keys()`'s result, comfortably below the current
    /// count (117 as of task 008) but far above what a broken derivation
    /// would produce — a regex or parser that stops matching yields a
    /// handful of keys or zero, never something close to the real count.
    /// Not `> 0`: a derivation that only finds three keys must fail as
    /// loudly as one that finds none (task 008 §4).
    const MIN_PLAUSIBLE_KEY_COUNT: usize = 100;

    /// Derives every key `tr_en` matches on, by reading `i18n.rs`'s own
    /// source text — not a hand-maintained sample (task 008). A key added
    /// to `tr_en` tomorrow is covered by both the parity test and the
    /// wording guard below without anyone remembering to list it.
    ///
    /// Scrapes match-arm key literals only, never values: every arm —
    /// whether its value is a single-line string or a multi-line `{ }`
    /// block — opens with the same `"key.name" => ` shape on its own
    /// line, so finding that shape is enough; nothing here needs to
    /// parse what the arm's value looks like. A value-only line (the
    /// second line of a block-form arm) starts with `"` too but has
    /// nothing after its closing quote, so it does not match.
    fn all_keys() -> Vec<&'static str> {
        let source = include_str!("i18n.rs");
        let body = source
            .split("fn tr_en(key: &str) -> &'static str {")
            .nth(1)
            .and_then(|rest| rest.split("\n}\n").next())
            .expect("tr_en function body");
        body.lines()
            .filter_map(|line| {
                let rest = line.trim_start().strip_prefix('"')?;
                let end = rest.find('"')?;
                let key = &rest[..end];
                rest[end + 1..]
                    .trim_start()
                    .starts_with("=>")
                    .then_some(key)
            })
            .collect()
    }

    #[test]
    fn rust_and_javascript_bridge_versions_match() {
        let lifecycle = include_str!("../js/src/lifecycle.js");
        let editor = include_str!("../js/src/editor.js");
        let bundle = include_str!("../assets/editor-bundle.js");
        assert_eq!(bekoedit_ui_contract::BRIDGE_SCHEMA_VERSION, 2);
        assert!(lifecycle.contains("BRIDGE_SCHEMA_VERSION = 2"));
        assert!(editor.contains("export { BRIDGE_SCHEMA_VERSION"));
        assert!(bundle.contains("protocolVersion:2"));
        assert!(bundle.contains("window.__bk="));
        assert!(bundle.contains("armFocusGuard"));
        assert!(bundle.contains("cancelFocusGuardsThrough"));
        assert!(bundle.contains("consumeFocusGuard"));
    }

    /// RFC-046 §3.1: `paste.js` pre-checks the pasted HTML's length before it
    /// ever crosses the bridge, so its threshold must not drift from the
    /// crate's own authoritative byte limit.
    #[test]
    fn paste_size_pre_check_matches_the_crate_limit() {
        let paste_js = include_str!("../js/src/paste.js");
        assert_eq!(bekoedit_paste::MAX_HTML_BYTES, 1024 * 1024);
        assert!(paste_js.contains("MAX_HTML_UTF16_LENGTH = 1024 * 1024"));
    }

    #[test]
    fn application_root_assets_are_cargo_native_and_current() {
        let app = include_str!("app.rs");
        let host = include_str!("source_sync/host.rs");
        let placeholder = "This should be replaced by dx";

        assert!(!crate::app::STYLE_SOURCE.trim().is_empty());
        assert!(crate::app::STYLE_SOURCE.contains(".shell"));
        assert!(!crate::app::STYLE_SOURCE.contains(placeholder));

        assert!(!crate::app::SHORTCUTS_SOURCE.trim().is_empty());
        assert!(crate::app::SHORTCUTS_SOURCE.contains("window.__bk_shortcut_relay"));
        assert!(!crate::app::SHORTCUTS_SOURCE.contains(placeholder));

        assert!(!app.contains("asset!(\"/assets/style.css\")"));
        assert!(!app.contains("asset!(\"/assets/shortcuts.js\")"));
        assert!(!host.contains("asset!(\"/assets/editor-bundle.js\")"));
    }

    #[test]
    fn i18n_all_keys_have_both_languages() {
        use crate::i18n::{Lang, tr};
        let all_keys = all_keys();
        assert!(
            all_keys.len() >= MIN_PLAUSIBLE_KEY_COUNT,
            "derived key set implausibly small ({} keys, expected at least {}) \
             — the tr_en scraper likely broke",
            all_keys.len(),
            MIN_PLAUSIBLE_KEY_COUNT
        );
        let mut missing = Vec::new();
        for key in &all_keys {
            if tr(Lang::En, key).is_empty() {
                missing.push(format!("EN missing: {key}"));
            }
            if tr(Lang::Ja, key).is_empty() {
                missing.push(format!("JA missing: {key}"));
            }
        }
        assert!(
            missing.is_empty(),
            "i18n coverage gaps:\n{}",
            missing.join("\n")
        );
    }

    /// Recursively collects `(relative_path, source)` for every `.rs` file
    /// under `dir`, labeling each with `label` so the relative path matches
    /// what a reader sees in the repository. Mirrors
    /// `tests/rfc_042.rs::collect_rust_sources`.
    fn collect_rust_sources(dir: &std::path::Path, label: &str, out: &mut Vec<(String, String)>) {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("read_dir({}): {error}", dir.display()));
        for entry in entries {
            let entry = entry.expect("directory entry is readable");
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                collect_rust_sources(&path, &format!("{label}/{file_name}"), out);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            out.push((format!("{label}/{file_name}"), source));
        }
    }

    /// Every **string-literal** key passed as `tr(<lang-expr>, "<key>")`
    /// across `src`, with the file it was found in (task 044 §2.2 -- a key
    /// used but defined in neither `tr_en` nor `tr_ja` is otherwise
    /// invisible to `all_keys()`, which only derives from `tr_en`'s own
    /// source). A call with a variable second argument (`tr(lang, key)`) is
    /// out of scope here; see the task's review request for that list.
    ///
    /// Line comments (`//`, `///`, `//!`) are dropped first, whole line, so
    /// a comment that merely mentions `tr(...)` as text is never scraped.
    /// Matching is on raw source bytes, not parsed tokens: a `tr(` is found
    /// at a word boundary (not preceded by an identifier character, so
    /// `tr_en(` never matches), its first argument is skipped to the next
    /// top-level comma (depth-tracked, so a parenthesized lang expression
    /// would not confuse it, though none exist today), and the key is
    /// scraped only when the very next non-whitespace byte is an
    /// unescaped `"` -- a string interpolated like `"{key}"` is not, and is
    /// correctly left for the dynamic-key list instead.
    fn literal_tr_keys() -> Vec<(String, String)> {
        let manifest_dir_string = std::env::var("CARGO_MANIFEST_DIR").expect("run by cargo test");
        let manifest_dir = std::path::Path::new(&manifest_dir_string);
        let mut sources = Vec::new();
        collect_rust_sources(&manifest_dir.join("src"), "src", &mut sources);

        let mut found = Vec::new();
        for (path, source) in &sources {
            // This project's test modules are trailing (`#[cfg(test)] mod
            // tests { ... }` at the end of the file, or a `mod tests;`
            // declaration pointing at one) -- never interleaved with
            // production code. Truncating here keeps a deliberate test
            // fixture like i18n.rs's own `tr(Lang::En, "nope.nope")` (which
            // tests the fallback for an undefined key, and must stay
            // undefined) from ever being scraped as a real call site.
            let production_source = match source.find("#[cfg(test)]") {
                Some(at) => &source[..at],
                None => source.as_str(),
            };
            let decommented: String = production_source
                .lines()
                .map(|line| {
                    if line.trim_start().starts_with("//") {
                        ""
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let bytes = decommented.as_bytes();
            let mut i = 0;
            while let Some(found_at) = decommented[i..].find("tr(") {
                let start = i + found_at;
                let preceded_by_ident = start > 0
                    && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
                i = start + "tr(".len();
                if preceded_by_ident {
                    continue;
                }
                // Skip the first argument to the next top-level comma.
                let mut depth = 1i32;
                let mut j = i;
                let mut comma_at = None;
                while j < bytes.len() && depth > 0 {
                    match bytes[j] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        b',' if depth == 1 => {
                            comma_at = Some(j);
                            break;
                        }
                        _ => {}
                    }
                    j += 1;
                }
                let Some(comma_at) = comma_at else { continue };
                let mut k = comma_at + 1;
                while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                    k += 1;
                }
                if bytes.get(k) != Some(&b'"') {
                    continue;
                }
                let mut end = k + 1;
                while end < bytes.len() && bytes[end] != b'"' {
                    if bytes[end] == b'\\' {
                        end += 1;
                    }
                    end += 1;
                }
                if end >= bytes.len() {
                    continue;
                }
                let key = &decommented[k + 1..end];
                found.push((key.to_string(), path.clone()));
            }
        }
        found
    }

    #[test]
    fn every_literal_key_passed_to_tr_is_defined_in_both_languages() {
        use crate::i18n::{Lang, tr};
        const MIN_PLAUSIBLE_CALL_SITE_COUNT: usize = 100;
        let sites = literal_tr_keys();
        assert!(
            sites.len() >= MIN_PLAUSIBLE_CALL_SITE_COUNT,
            "derived call-site count implausibly small ({} sites, expected at \
             least {}) -- the literal_tr_keys scraper likely broke",
            sites.len(),
            MIN_PLAUSIBLE_CALL_SITE_COUNT
        );
        let mut missing = Vec::new();
        for (key, file) in &sites {
            if tr(Lang::En, key).is_empty() {
                missing.push(format!("EN missing: {key} (used in {file})"));
            }
            if tr(Lang::Ja, key).is_empty() {
                missing.push(format!("JA missing: {key} (used in {file})"));
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "a key literally passed to tr() is undefined in at least one \
             language:\n{}",
            missing.join("\n")
        );
    }

    /// Developer jargon that must not leak into user-visible strings
    /// (RFC-041 §4, DEC-015) — internal terminology stays precise
    /// (`ConflictState`, `RawIsland`, `SourcePatch`); what the user reads
    /// uses plain language. A short explicit list, not a heuristic: a
    /// clever detector produces false positives, and false positives get
    /// "fixed" by weakening the detector until it enforces nothing
    /// (task 007 §2). Case-insensitive against the English arm; the
    /// Japanese arm has its own list below it, not a translation of this
    /// one — the failure mode there is a katakana loanword, not the
    /// English word itself.
    const JARGON_EN: &[&str] = &[
        "patch",
        "buffer",
        "serialize",
        "deserialize",
        "mutex",
        "thread",
        "async",
        "signal",
        "widget",
        "DOM",
        "WebView",
        "bridge",
        "protocol",
        "revision",
        "fingerprint",
        "epoch",
        "snapshot",
        "island",
        "canonical",
        "projection",
        "reducer",
        "invariant",
        "ELOC",
        "RFC",
    ];

    const JARGON_JA: &[&str] = &[
        "パッチ",         // patch
        "バッファ",       // buffer
        "シリアライズ",   // serialize
        "デシリアライズ", // deserialize
        "ミューテックス", // mutex
        "スレッド",       // thread
        "非同期",         // async
        "シグナル",       // signal
        "ウィジェット",   // widget
        "DOM",
        "WebView",
        "ブリッジ",           // bridge
        "プロトコル",         // protocol
        "リビジョン",         // revision
        "フィンガープリント", // fingerprint
        "エポック",           // epoch
        "スナップショット",   // snapshot
        "アイランド",         // island
        "キャノニカル",       // canonical
        "プロジェクション",   // projection
        "リデューサー",       // reducer
        "インバリアント",     // invariant
        "ELOC",
        "RFC",
    ];

    /// Per-key exceptions: a blocklisted term is allowed to appear in this
    /// one key's value, and nowhere else — removing a term from
    /// `JARGON_EN`/`JARGON_JA` entirely because one string needs it would
    /// disable the check everywhere (task 007 §3). Adding an entry here is
    /// a deliberate act; both entries below are justified at the point of
    /// use, not asserted without reasoning.
    const JARGON_EXCEPTIONS: &[(&str, &str)] = &[
        // Names a thing the user is being offered back by its product
        // name (a "recovery snapshot" — bekoedit_fs::RecoverySnapshot),
        // not an internal storage mechanism described to the user.
        ("recovery.description", "snapshot"),
        ("recovery.description", "スナップショット"),
        // "Raw Markdown Islands" is bekoedit's own public feature name,
        // documented in the README itself — not accidental internal
        // vocabulary leaking through.
        ("status.islands_hint", "island"),
        ("status.islands_hint", "アイランド"),
        // "Microsoft Edge WebView2 Runtime" is the product the user must
        // install, named as Microsoft names it on its download page -- the
        // one thing the message exists to tell them (task 020 §2).
        ("webview2.missing.body", "WebView"),
    ];

    #[test]
    fn visible_strings_use_plain_language() {
        use crate::i18n::{Lang, tr};
        let all_keys = all_keys();
        assert!(
            all_keys.len() >= MIN_PLAUSIBLE_KEY_COUNT,
            "derived key set implausibly small ({} keys, expected at least {}) \
             — the tr_en scraper likely broke",
            all_keys.len(),
            MIN_PLAUSIBLE_KEY_COUNT
        );

        let mut offenders = Vec::new();
        for key in &all_keys {
            let en = tr(Lang::En, key).to_lowercase();
            for term in JARGON_EN {
                if en.contains(&term.to_lowercase()) && !JARGON_EXCEPTIONS.contains(&(*key, *term))
                {
                    offenders.push(format!("EN {key}: contains {term:?}"));
                }
            }
            let ja = tr(Lang::Ja, key);
            for term in JARGON_JA {
                if ja.contains(term) && !JARGON_EXCEPTIONS.contains(&(*key, *term)) {
                    offenders.push(format!("JA {key}: contains {term:?}"));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "developer jargon in visible strings:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn pending_recovery_is_detected_for_startup_screen() {
        use bekoedit_core::AppState;
        use bekoedit_fs::{RecoverySnapshot, RecoveryStore};

        let dir = tempfile::tempdir().unwrap();
        let recovery = RecoveryStore::at(dir.path().join(".recovery"));
        let state = AppState::new(recovery.clone(), dir.path().join(".recent.json"), 100);
        assert!(!crate::app::has_pending_recovery(&state));
        recovery
            .save(&RecoverySnapshot {
                original_path: dir.path().join("doc.md"),
                text: "# recovered\n".into(),
                revision: 2,
                created_at_secs: 1,
            })
            .unwrap();
        assert!(crate::app::has_pending_recovery(&state));
        assert!(crate::app::should_show_recovery(&state, true, false));
        assert!(!crate::app::should_show_recovery(&state, false, false));
        assert!(!crate::app::should_show_recovery(&state, true, true));

        let mut active = state;
        active.new_untitled();
        assert!(!crate::app::should_show_recovery(&active, true, false));
    }

    #[test]
    fn owner_feedback_ui_contracts_are_present() {
        let start = include_str!("components/start_screen.rs");
        let app_bar = include_str!("components/app_bar.rs");
        let header = include_str!("components/editor_header.rs");
        let table_view = include_str!("components/form_mode/table_view.rs");
        let toast = include_str!("components/toast.rs");
        let style = include_str!("../assets/style.css");

        assert!(start.contains("submit_source_interaction"));
        assert!(!start.contains("state.write().new_untitled()"));
        assert!(app_bar.contains("data-source-focus-launch\": \"appbar-new"));
        assert!(header.contains("data-source-focus-launch\": \"mode-split"));
        assert!(header.contains("mode.close_split"));
        assert!(!header.contains("if has_workspace"));
        assert!(header.contains("if backlinks_available"));
        assert!(header.contains("search_open.set(false)"));
        assert!(!include_str!("components/search_panel.rs").contains("search.no_results"));
        assert!(!include_str!("components/search_panel.rs").contains("search.title"));
        assert!(include_str!("components/search_panel.rs").contains("autofocus: true"));
        assert!(include_str!("components/search_panel.rs").contains("results.set(Vec::new())"));
        assert!(include_str!("components/search_panel.rs").contains("searched.set(false)"));
        assert!(include_str!("components/search_panel.rs").contains("search.close"));
        assert!(include_str!("components/explorer.rs").contains("SearchOpen"));
        assert!(include_str!("components/explorer.rs").contains("SearchPanel {}"));
        assert!(include_str!("components/explorer/tree_row.rs").contains("is_markdown_path"));
        // RFC-042 §7.4/§11 (slice 2): the native `disabled` attribute was
        // reverted — a disabled row leaves the tab order and
        // assistive-technology focus entirely, which contradicts the tree
        // pattern. Must never reappear anywhere in the row renderer.
        assert!(
            !include_str!("components/explorer/tree_row.rs").contains("disabled: !is_openable")
        );
        assert!(include_str!("components/explorer.rs").contains("workspace-new-file-name"));
        assert!(include_str!("components/explorer.rs").contains("search.label"));
        assert!(include_str!("state.rs").contains("pub enum OpenMenu"));
        assert!(app_bar.contains("stop_propagation"));
        assert!(header.contains("stop_propagation"));
        assert_eq!(
            crate::i18n::tr(crate::i18n::Lang::En, "backlinks.title"),
            "Linked from"
        );
        assert!(header.contains("class: \"adv-menu-wrap\""));
        // RFC-048 slice 2: the table grid moved into its own component
        // (table_view.rs), so AddIcon is used there now, not inline in
        // block_view.rs's match.
        assert!(table_view.contains("AddIcon {}"));
        assert!(include_str!("state.rs").contains("pub struct SettingsOpen"));
        assert!(!include_str!("app.rs").contains("use_context::<Signal<bool>>"));
        assert!(toast.contains("fn ToastItem"));
        assert!(toast.contains("toast.dismiss"));
        assert!(style.contains(".mode-tab.active"));
        assert!(style.contains("--surface: #ffffff"));
        assert!(style.contains(".adv-menu-wrap { position: relative"));
        assert!(style.contains("position: absolute; inset: 48px 8px 8px"));
        assert!(style.contains("width: min(200px, calc(100vw - 16px)); min-width: 0"));
        assert!(style.contains("width: min(180px, calc(100vw - 16px)); min-width: 0"));
        let app_menu_rule = style
            .split(".app-bar-dropdown {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .unwrap();
        let advanced_menu_rule = style
            .split(".adv-dropdown {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .unwrap();
        assert!(!app_menu_rule.contains("min-width: 200px"));
        assert!(!advanced_menu_rule.contains("min-width: 180px"));
        let used_width = |preferred: u32, viewport: u32| preferred.min(viewport.saturating_sub(16));
        assert_eq!(used_width(200, 120), 104);
        assert_eq!(used_width(180, 120), 104);
    }

    /// Task 047 Part B: the inline-formatting toolbar used to install a
    /// brand-new `window.__bk_form_relay` plus a `while (true)` keep-alive
    /// loop on every click, rebinding the one over the last without ever
    /// releasing it -- each click left its predecessor's query running
    /// forever, accumulating over a session. The fix reads the selection
    /// through `bridge::eval_body`'s bounded, release-after-recv, never
    /// `join`ed one-shot instead, which has nothing left to accumulate.
    #[test]
    fn inline_toolbar_reads_selection_through_the_one_shot_eval_not_a_persistent_relay() {
        let toolbar = include_str!("components/form_mode/inline_toolbar.rs");
        assert!(
            !toolbar.contains("__bk_form_relay"),
            "the old per-click relay must be gone entirely: {toolbar}"
        );
        assert!(
            !toolbar.contains("while(true)") && !toolbar.contains("while (true)"),
            "no click handler may install its own keep-alive loop: {toolbar}"
        );
        assert!(
            toolbar.contains("crate::bridge::eval_body::<Option<Sel>>(&js)"),
            "a click must read the selection through the shared one-shot eval: {toolbar}"
        );
    }

    #[test]
    fn task_005_settings_layer_cleanup_contracts() {
        let fs_lib = include_str!("../../bekoedit-fs/src/lib.rs");
        let fs_settings = include_str!("../../bekoedit-fs/src/settings.rs");
        let app_settings = include_str!("settings.rs");
        let persistence = include_str!("persistence.rs");
        let settings_screen = include_str!("components/settings_screen.rs");
        let app = include_str!("app.rs");

        // Part B: the five dead persistence functions are gone; the
        // UserSettings type itself stays exported.
        assert!(!fs_settings.contains("fn default_path"));
        assert!(!fs_settings.contains("fn load("));
        assert!(!fs_settings.contains("fn save("));
        assert!(!fs_settings.contains("fn load_user_settings"));
        assert!(!fs_settings.contains("fn save_user_settings"));
        assert!(fs_lib.contains("pub use settings::UserSettings;"));

        // Part A: the fallback is a path-plus-flag return, not a bare
        // `unwrap_or_else` — the information used_temp_fallback carries no
        // longer has anywhere to be silently discarded.
        assert!(!app_settings.contains("unwrap_or_else(std::env::temp_dir)"));
        assert!(app_settings.contains("used_temp_fallback"));
        assert!(persistence.contains("fn settings_used_temp_fallback"));
        assert!(app.contains("settings_used_temp_fallback()"));
        assert!(app.contains("ToastKind::Warning"));
        assert!(app.contains("settings.temp_fallback_warning"));

        // Part C: save failures are propagated, not swallowed with `let _
        // =`, and reach the user through the toast layer.
        assert!(app_settings.contains("pub fn save(&self) -> std::io::Result<()>"));
        assert!(app_settings.contains("pub fn save_to(&self, path: &Path) -> std::io::Result<()>"));
        assert!(!app_settings.contains("let _ = bekoedit_fs::atomic_write"));
        assert!(persistence.contains(
            "pub fn save_settings(&self, settings: &AppSettings) -> std::io::Result<()>"
        ));
        assert!(settings_screen.contains("if let Err(err) = persistence.save_settings(&s)"));
        assert!(settings_screen.contains("ToastKind::Error"));
        assert!(settings_screen.contains("settings.save_failed"));

        // Re-review §2 correction: on a failed save the screen must stay
        // open, not close — `settings` is component-local, so closing
        // would unmount it, and reopening reloads the old values from
        // disk, discarding the user's edits after only the toast's
        // 4-second auto-dismiss. The failure branch returns early; the
        // success-only actions (applying the live language/mode, closing)
        // come after it, not inside it.
        let err_branch = settings_screen
            .split("if let Err(err) = persistence.save_settings(&s) {")
            .nth(1)
            .and_then(|rest| rest.split("}\n").next())
            .expect("save-button Err branch");
        assert!(err_branch.contains("return;"));
        assert!(!err_branch.contains("close_settings()"));
    }

    #[test]
    fn rfc_042_slice_5_form_mode_blocks_are_named_groups() {
        let block_view = include_str!("components/form_mode/block_view.rs");

        // §5.1: every rendered block is a role="group" carrying an
        // accessible name — asserted once, since the role/aria_label pair
        // sits on the one wrapper shared by every variant, not duplicated
        // per arm.
        assert!(block_view.contains(r#"role: "group""#));
        assert!(block_view.contains(r#"aria_label: "{group_name}""#));

        // Per variant, not once for the file (§7.1): each of the eight
        // non-island kinds names itself from its own translated key, so a
        // variant added later without one is caught by this list falling
        // short rather than by a single file-wide substring match.
        for key in [
            "block.kind.heading",
            "block.kind.paragraph",
            "block.kind.blockquote",
            "block.kind.list",
            "block.kind.code",
            "block.kind.horizontal_rule",
            "block.kind.table",
            "block.kind.image",
        ] {
            assert!(
                block_view.contains(&format!("tr(lang, \"{key}\").to_string()")),
                "missing group-name key: {key}"
            );
        }

        // §5.2: RawIsland's group name reuses label_key (the existing
        // per-type island reason) and island.hint — no new literal reason
        // text. Checked as the exact format! expression, not as two loose
        // substring checks: `tr(lang, &label_key)` and
        // `tr(lang, "island.hint")` each already appear elsewhere in this
        // file (the pre-existing visible island-label/island-hint spans),
        // so either alone would pass even if the group name itself used
        // neither — only the full expression pins down what this slice
        // actually added.
        assert!(
            block_view
                .contains(r#"format!("{}: {}", tr(lang, &label_key), tr(lang, "island.hint"))"#)
        );

        // §7.3: the new keys are picked up by task 008's derived key set
        // automatically — not re-listed in ALL_KEYS by hand. Spot-checks
        // the already-derived result rather than maintaining a second
        // parallel list of the same keys.
        let derived = all_keys();
        let block_kind_keys: Vec<&&str> = derived
            .iter()
            .filter(|k| k.starts_with("block.kind."))
            .collect();
        assert_eq!(
            block_kind_keys.len(),
            8,
            "expected 8 block.kind.* keys picked up automatically, found {block_kind_keys:?}"
        );
    }
}
