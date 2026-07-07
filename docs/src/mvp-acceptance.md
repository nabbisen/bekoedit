# MVP Acceptance Checklist

This is the formal gate for bekoedit v1.0.0.

**The only requirement before tagging v1.0.0 is maintainer sign-off on the
three blocking items below. Everything else is either already proven by
automated tests, or a documented known limitation that can be addressed in
point releases.**

---

## Blocking items (sign-off required)

- [ ] **Manual walkthrough** — the maintainer has personally run the ten-minute
  scenario in the appendix on the release binary for at least one of the target
  platforms (macOS, Linux, or Windows).
- [ ] **No known data-loss bugs** — there are no open issues tagged
  `data-loss` or `source-corruption` at the time of release.
- [ ] **Release artifacts build cleanly on CI** — the release workflow
  produces binaries for all three target platforms without errors.

---

## Automated (already proven — no manual action needed)

These pass on every push. They are not blocking gates because the CI already
enforces them.

| Concern | Test | Location |
|---------|------|---------|
| Source preservation — patches touch only target region | 118 unit tests | `form_tests/basic_tests.rs`, `inline_tests.rs`, `table_tests.rs` |
| UTF-8 boundary patches impossible | `invalid_utf8_boundary_rejected` | `form_tests/basic_tests.rs` |
| Raw islands never silently changed | `raw_island_edit_patches_only_the_island` | same |
| Stale revision/fingerprint rejected | `stale_revision_is_rejected`, `fingerprint_mismatch_is_rejected` | same |
| Atomic save — no partial writes | `atomic_write_*` | `bekoedit-fs/src/tests.rs` |
| Path traversal blocked | `traversal_*` | same |
| Conflict detection — disk change during dirty edit | smoke test check 4 | `smoke_test.rs` |
| Full benchmark — 3.57 ms reparse on 240 KB doc | RFC-032 bench | `benches/reparse.rs` |
| Headless smoke test — all 5 core paths | `--headless-smoke` | CI post-build step |

---

## Known limitations (documented, not blocking)

These are real constraints. They are listed here so users know what to expect,
not because they block the release.

| Limitation | Impact | Planned |
|-----------|--------|---------|
| **IME composition not covered by automated tests** | CJK users should test with their preferred input method before relying on the app for production work. CodeMirror 6 handles IME natively; no known bugs, but not formally verified. | Point release if bugs reported |
| **No paid code signing** | macOS Gatekeeper and Windows SmartScreen show an "unidentified developer" warning. See `docs/src/distribution.md` for the one-time bypass procedure. | Optional future investment |
| **No incremental parsing** | Reparse after edit is always full (3.57 ms median; adequate for current document sizes). Very large workspaces (>500 KB documents) may feel slower. | RFC-032, deferred until profiling shows need |
| **No split-pane scroll sync tests** | Scroll synchronisation between Text Mode and Preview in Split Mode is implemented but has no automated test (requires real DOM measurements). | Manual QA step in walkthrough |
| **Single document open** | Only one Markdown file is open at a time. No multi-tab interface. | Post-1.0 roadmap |

---

## Appendix: ten-minute release walkthrough

A pass of this scenario on the release binary counts as completing the
"manual walkthrough" gate above.

1. Launch the app. Start screen appears.
2. Open a folder that contains at least three Markdown files.
3. File tree shows the Markdown files; non-Markdown files are hidden.
4. Open a file. Text Mode appears with syntax highlighting.
5. Edit a paragraph. Save state shows "Unsaved changes", then "Saved" after autosave.
6. Switch to Form Mode. Paragraph shows as an editable block.
7. Edit a heading via Form Mode. Switch back to Text Mode; only the heading changed.
8. Switch to Split Mode. Edit in Text pane; preview updates.
9. Open a second terminal. Overwrite the open file externally. Conflict banner appears; choose "Keep my version"; file is saved successfully.
10. Create a new file. Type a name; click Create. File appears in the tree and opens.
11. Rename the file. New name appears in the tree and the editor header.
12. Delete the file. Confirmation dialog appears; confirm; file is removed from the tree.
13. Open the Outline panel. Click a heading; editor scrolls to it.
14. Close the app.

All thirteen steps should complete without errors or unexpected data changes.

---

## Sign-off

**Maintainer: @nabbisen**

```
[ ] Walkthrough completed on: __________ (platform: __________)
[ ] No open data-loss issues
[ ] CI release build passes

Signed: _________________________ Date: _____________
```
