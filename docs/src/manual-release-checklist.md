# Manual Release Checklist

Use this checklist for final human release sign-off. Copy it into the
release-candidate evidence record, then replace placeholders with observed
results. Do not mark an item done unless it was observed on a release artifact
binary, not a development build.

## Candidate

| Field | Value |
|-------|-------|
| Version | `X.Y.Z` |
| Tag | `X.Y.Z` |
| Commit | `________` |
| Release workflow run | `________` |
| Artifact directory | `________` |
| Tester | `________` |
| Platform | `________` |
| Date | `YYYY-MM-DD` |

## Setup

- [ ] Extract the release archive for the tested platform into a fresh
      temporary directory.
- [ ] Launch the extracted release binary, not `target/release/bekoedit`.
- [ ] Create or copy a disposable Markdown workspace for testing.
- [ ] Keep a copy of the original Markdown file for byte/source comparison.

Notes:

```text
Archive tested:
Extraction path:
Workspace path:
Original file path:
```

## Launch And Workspace

- [ ] App launches without panic or immediate exit.
- [ ] Start screen appears.
- [ ] Open workspace succeeds.
- [ ] File tree shows Markdown files and ignores expected noise directories.
- [ ] Opening an existing Markdown file shows the expected document.

Evidence / notes:

```text
________
```

## Text Mode Editing

- [ ] Switch to Text Mode.
- [ ] Edit normal ASCII text.
- [ ] Edit multibyte text, for example Japanese text or emoji.
- [ ] Save.
- [ ] Close and reopen the document.
- [ ] Saved text is present and not duplicated or truncated.

Evidence / notes:

```text
________
```

## Form Mode Editing

- [ ] Switch to Form Mode.
- [ ] Edit a heading.
- [ ] Edit a paragraph.
- [ ] Edit a supported list item.
- [ ] Toggle a task checkbox if the fixture contains one.
- [ ] Edit a simple table cell if the fixture contains a simple table.
- [ ] Save.
- [ ] Reopen in Text Mode and confirm the intended source changed.
- [ ] Confirm unrelated Markdown source remains unchanged.

Evidence / notes:

```text
________
```

## Raw Markdown Islands

- [ ] Open or create a document containing at least one unsupported structure,
      such as front matter, raw HTML block, complex table, or math block.
- [ ] Confirm unsupported content is presented as a raw island.
- [ ] Edit inside the raw island.
- [ ] Save and reopen.
- [ ] Confirm the raw island content is preserved except for the intended edit.
- [ ] Confirm supported Form Mode edits do not silently normalize/delete raw
      island content.

Evidence / notes:

```text
________
```

## Source Preservation Spot Check

- [ ] Use a fixture containing at least CRLF or mixed Markdown syntax.
- [ ] Save after one small edit.
- [ ] Compare before/after source manually or with a diff tool.
- [ ] Confirm only the intended source range changed.
- [ ] Confirm line endings and unrelated syntax are preserved.

Evidence / notes:

```text
Diff command/tool:
Result:
```

## Conflict Handling

- [ ] Open a document in bekoedit.
- [ ] Make an unsaved edit in the app.
- [ ] Modify the same file externally with another editor or shell.
- [ ] Attempt to save in bekoedit.
- [ ] Confirm conflict is surfaced and save is blocked.
- [ ] Confirm neither in-memory nor disk version is silently lost.
- [ ] Resolve by reload / keep mine / save copy, whichever is available and
      appropriate for the test.

Evidence / notes:

```text
________
```

Result:

```text
Conflict handling result: pass / fail / deferred
Decision and reason if deferred:
Accepted by:
Follow-up:
```

## Recovery Presentation

- [ ] Create a dirty document state.
- [ ] Force-close the app or otherwise simulate an interrupted session.
- [ ] Relaunch the release binary.
- [ ] Confirm pending recovery is presented on startup if a recovery snapshot
      exists.
- [ ] Test recover action.
- [ ] Test discard action on a separate disposable snapshot, if practical.

Evidence / notes:

```text
________
```

Result:

```text
Recovery presentation result: pass / fail / deferred
Decision and reason if deferred:
Accepted by:
Follow-up:
```

## Keyboard And Accessibility Smoke

- [ ] Use keyboard shortcuts to switch modes.
- [ ] Use keyboard shortcut to save.
- [ ] Navigate file tree or primary controls without mouse where practical.
- [ ] Confirm visible save/conflict status changes are understandable.

Evidence / notes:

```text
________
```

## IME Composition

- [ ] Use a real IME, for example Japanese input.
- [ ] Compose text in Text Mode.
- [ ] Confirm partial composition text is not prematurely committed or
      duplicated.
- [ ] Commit composition.
- [ ] Save and reopen.
- [ ] Confirm committed text is preserved exactly.

If IME testing is deferred, record the decision explicitly:

```text
IME result: pass / fail / deferred
Input method:
Platform:
Decision and reason if deferred:
```

## Release Blocker Scan

- [ ] Confirm there are no known open issues tagged `data-loss`.
- [ ] Confirm there are no known open issues tagged `source-corruption`.
- [ ] Confirm no manual test failure above remains unexplained.
- [ ] Confirm accepted risks are documented below.

Evidence / notes:

```text
Issue tracker query or URL:
Search date:
Result:
```

## Accepted Risks

| Risk | Decision | Follow-up |
|------|----------|-----------|
| Unsigned binaries | `accepted / blocked` | `________` |
| `cargo audit` allowed warnings | `accepted / blocked` | `________` |
| Manual test gaps | `none / accepted / blocked` | `________` |
| IME if deferred | `accepted / blocked / n/a` | `________` |

## Final Manual Sign-Off

```text
[ ] Launch/workspace test passed.
[ ] Text Mode test passed.
[ ] Form Mode test passed.
[ ] Raw island/source preservation spot check passed.
[ ] Conflict handling test passed or deferral is explicitly accepted.
[ ] Recovery presentation test passed or deferral is explicitly accepted.
[ ] Keyboard/accessibility smoke passed.
[ ] IME composition passed or deferral is explicitly accepted.
[ ] No data-loss/source-corruption blocker is known.
[ ] Accepted risks are recorded.

Release decision: release / block / defer
Signed:
Date:
```
