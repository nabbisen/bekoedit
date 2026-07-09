# MVP Acceptance Checklist

**No v1.0.0 release without explicit maintainer sign-off on every item.**

Each item links to its evidence. Items marked ✅ have code, tests, or
documentation evidence in the repository. Items marked ⚠️ require release
candidate verification or latest CI/release-run evidence before v1.0.0
sign-off.

---

## Source preservation (RFC-013/014/015)

- ✅ Adversarial golden document: editing one block leaves every other byte
  unchanged — CRLF, Japanese/emoji, tilde fences, non-1 ordered lists,
  reference links, front matter, raw HTML.
  *Evidence: `tests::adversarial_tests::adversarial_*` (8 tests)*
- ✅ Form Mode patches are <50 ms round-trip on a 10 000-word document.
  *Evidence: RFC-032 benchmark, 3.57 ms on 240 KB release build*
- ✅ UTF-8 boundary patches are impossible; invalid offsets return
  `FormEditError::InvalidEditPayload`.
  *Evidence: `form_tests::basic_tests::multibyte_form_edit_is_utf8_safe`*
- ✅ Raw Markdown Islands are never silently normalised or deleted.
  *Evidence: `raw_island_edit_patches_only_the_island`,
  `structured_edit_on_island_is_rejected`*

## Document lifecycle (RFC-006/007/008)

- ✅ Autosave writes atomically; killing mid-save leaves the file either
  fully written or untouched.
  *Evidence: `atomic_write_*` tests in `bekoedit-fs/src/tests.rs`*
- ✅ External modifications are detected (fs watcher + pre-save check).
  *Evidence: headless smoke test check 4 (`ConflictState::DiskChangedDirtyMemory`)*
- ✅ Neither disk version nor in-memory version is silently lost on conflict.
  *Evidence: `conflict_resolution_*`,
  `pending_conflict_blocks_section_moves`, and
  `pending_conflict_blocks_history_restore` tests in
  `bekoedit-core/src/tests.rs`*
- ✅ Crash-recovery snapshot exists before risky writes; presented on next launch.
  *Evidence: `recovery_snapshot_*` tests*
- ✅ Save failures surface to the user via the assertive live region.
  *Evidence: `status_bar.rs` — `SaveState::SaveFailed` → `role="alert"`*

## File operations (RFC-003/004/005)

- ✅ Path traversal outside workspace root is rejected for all file operations.
  *Evidence: `traversal_*` tests in `bekoedit-fs/src/tests.rs`*
- ✅ Deletion goes to system trash by default; permanent requires
  `DeleteStrategy::Permanent`.
  *Evidence: `delete_path` signature in `store_file_ops.rs`*
- ✅ Renaming the open document updates the session path without data loss.
  *Evidence: `rename_path` in `store_file_ops.rs` updates `session.path`*
- ✅ Deleting a dirty open document is blocked until resolved.
  *Evidence: `delete_dirty_document_returns_document_dirty` test*

## Editing modes (RFC-010/011/016/019)

- ✅ Switching modes does not alter the canonical Markdown source.
  *Evidence: mode switch only changes `Signal<EditorMode>` — no write path*
- ✅ Text Mode does not send partial composition text to Rust during CJK input.
  `compositionstart` cancels the debounce timer; `compositionend` flushes
  once composition is committed.
  *Evidence: `editor.js` `compositionstart`/`compositionend` DOM event handlers;
  CM6 bundle rebuilt 2026-06-07.*
  *Note: functional IME behaviour still requires manual smoke-test on release
  (no headless CJK keyboard available in CI). Mark the walkthrough item below
  when complete.*
- ✅ Form Mode sends only semantic commands; whole-document rewrite
  is impossible from Form Mode.
  *Evidence: `resolve_form_edit` only accepts `FormBlockEdit` variants*
- ✅ Outline panel navigation scrolls CodeMirror to the correct heading.
  *Evidence: `outline_panel.rs` dispatches CM6 `scrollIntoView` via eval*

## Accessibility (RFC-021)

- ✅ All primary workflows completable with keyboard only.
  *Evidence: Ctrl+S/1/2/3/4/B/F shortcuts in `shortcuts.js`*
- ✅ File tree exposes `role="tree"` / `role="treeitem"` with `aria-selected`.
  *Evidence: `explorer.rs` lines 3–4, 110*
- ✅ Save status changes announced via polite live region.
  *Evidence: `status_bar.rs` — `role="status"` + `aria-live="polite"`*
- ✅ Save failures announced via assertive live region.
  *Evidence: `status_bar.rs` — `role="alert"` + `aria-live="assertive"`*

## Internationalisation (RFC-022)

- ✅ All UI keys have both EN and JA translations; missing keys detected at
  test time, not runtime.
  *Evidence: `tests::app_tests::i18n_all_keys_have_both_languages`*
- ✅ Language switch works at runtime.
  *Evidence: `Signal<Lang>` propagated via context*

## CI / distribution (RFC-024/025)

- ⚠️ CI lint + test + build are configured on push and pull request.
  *Evidence: `.github/workflows/ci.yml`; release sign-off must inspect the
  latest CI run.*
- ⚠️ Headless smoke test (`--headless-smoke`) is configured as a blocking CI
  gate and exercises 5 checks.
  *Evidence: `.github/workflows/ci.yml`, `smoke_test.rs`; release sign-off
  must inspect the latest CI run.*
- ⚠️ Release workflow is configured to build Linux/macOS/Windows artifacts on
  tag push.
  *Evidence: `.github/workflows/release.yml`; release sign-off must inspect
  the produced artifacts, including Windows zip root layout.*
- ✅ Unsigned binary guidance documented.
  *Evidence: `docs/src/distribution.md`*

---

## Known limitations (not blocking)

| Limitation | Planned |
|-----------|---------|
| IME composition not covered by automated tests | Manual QA on release; CodeMirror 6 handles it natively |
| No paid code signing | Optional future investment |
| Full reparse only (3.57 ms — adequate) | RFC-032, deferred |
| No multi-tab | Post-1.0 roadmap |

---

## Maintainer sign-off

Before pushing the v1.0.0 tag, sign here:

```
[ ] I have run the ten-minute release walkthrough on the release binary.
    Platform: ____________   Date: ____________

[ ] IME composition verified with ____________ input method on ____________.
    (or: deferred to post-1.0 based on community reports)

[ ] No open issues tagged `data-loss` or `source-corruption`.

[ ] CI release workflow produced artifacts for all three platforms.

Signed: @nabbisen   Date: ____________
```
