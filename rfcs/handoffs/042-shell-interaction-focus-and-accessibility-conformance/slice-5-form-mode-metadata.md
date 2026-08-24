# RFC-042 handoff — slice 5: Form Mode block metadata

**Governing RFC:** [RFC-042](../../done/RFC-042-shell-interaction-focus-and-accessibility-conformance.md) §8, and §13.1 which records this slice as scheduled
**Slice:** 5 of 7 — **the last one. RFC-042 closes when this lands.**
**Baseline:** `main` at `25143bb`
**Status:** inherited from RFC-042 (Implemented). Historical execution guide — see RFC-042 §13.1 for this slice's disposition.
**Date:** 2026-08-10

---

## 0. How to read this handoff

Sections are marked **[Binding]** or **[Advisory]**. Binding is mine to decide;
advisory is a suggestion you may replace with reasoning, without asking.

## 1. Merge gate — read before starting · **[Binding]**

`main` carries the prepared 0.14.0 release (`25143bb`), and **no `0.14.0` tag
exists yet**. That release's CHANGELOG and ROADMAP both state, deliberately,
that Form Mode block editing does *not* expose accessibility metadata.

**This slice must not merge to `main` until the `0.14.0` tag exists on the
remote.** If it merged first, the tag would contain work the release notes
explicitly say it does not, and those notes were written to be honest about
precisely this gap.

You may branch, implement, push, open a draft PR, and let CI run — none of that
touches `main`. Before merging, **verify the tag exists** (`git ls-remote --tags
origin 0.14.0`) and say so in the review request. If it does not, stop and
report; do not wait-and-retry.

## 2. Purpose

Form Mode is one of three primary editing modes and exposes no accessibility
metadata at all (RFC-042 F-8). A screen-reader user editing there meets a
sequence of unlabelled fields with no way to tell a heading from a paragraph
from a code block.

Everything around it now conforms — tree, menus, tabs, conflict banner,
Recovery, Settings. This is the surface that makes the theme coherent or leaves
it lopsided.

## 3. Background

`components/form_mode/block_view.rs` (266 ELOC) renders ten
`FormBlockDisplay` variants: `Heading`, `Paragraph`, `Blockquote`, `List`,
`Code`, `HorizontalRule`, `Table`, `Image`, `RawIsland`.

Island reason strings already exist and are already translated —
`island.front_matter`, `island.html_block`, `island.complex_table`,
`island.math_block`, `island.directive`, `island.complex_list`,
`island.complex_blockquote`, `island.unknown_extension`,
`island.malformed_region`, `island.footnote`.

## 4. Change scope · **[Advisory]**

- `crates/bekoedit-app/src/components/form_mode/block_view.rs`
- `crates/bekoedit-app/src/components/form_mode.rs` — only if the block
  container needs a role
- `crates/bekoedit-app/src/i18n.rs` — block-kind names, EN and JA
- `crates/bekoedit-app/src/tests.rs` — guard assertions

## 5. Required implementation

### 5.1 A named group per block · **[Binding]**

Each rendered block is a `role="group"` carrying an accessible name that
includes its **kind**, from a translated key. Not `role="region"` — that is a
landmark, and ten landmarks per document is noise. Not `<fieldset>` — it carries
layout and styling baggage this slice must not introduce.

The name must survive translation: no literal strings in RSX, both language arms,
per §8's existing rule.

### 5.2 Raw islands say what they are and why · **[Binding]**

A `RawIsland` block's accessible name states both that it is a raw region and
the reason it could not be form-edited — **reusing the existing `island.*`
strings**. Do not write new reason text; those ten keys are already the
product's vocabulary for this and are already translated.

This is the one block kind where the *why* matters as much as the *what*: a
user who cannot see the highlighting needs to know the region is verbatim and
that editing it is unmediated.

### 5.3 Position is deliberately out of scope · **[Binding]**

Do not add "block 3 of 12" or similar. It would help orientation, and §8 does
not ask for it; adding it means deciding what happens when blocks are inserted,
deleted, or reordered mid-session, which is a design question this slice is not
opening. If a real user reports disorientation, that is the trigger to revisit.

### 5.4 No behaviour change · **[Binding]**

Metadata only. Nothing about editing, commit-on-blur, the delete buttons, the
inline toolbar, block resolution, or patch generation changes. If a change
appears to require touching any of those, stop and report.

## 6. Non-change scope · **[Binding]**

- `bekoedit-markdown`'s `FormProjection`, `FormBlockDisplay`, or block
  resolution — this slice reads what is already there.
- Slices 1–4 surfaces and the `shell_focus` helpers.
- Focus movement of any kind. Form Mode blocks are persistent content, not a
  focus-owning surface under §6.3; this slice acquires and releases nothing.
- `source_sync/`, `bekoedit-core`, `bekoedit-fs`, `bekoedit-ui-contract`.
- The release files — `CHANGELOG.md`, `ROADMAP.md`, `Cargo.toml`. Their 0.14.0
  content is correct and this slice is not in that release.

## 7. Required tests · **[Binding coverage, Advisory organization]**

Static metadata is what guard assertions genuinely verify, so this slice is
well covered by the existing technique.

1. Every `FormBlockDisplay` variant renders a `role="group"` with a translated
   name — assert per variant, not once for the file, so a variant added later
   without a name is caught.
2. The `RawIsland` arm references the `island.*` keys rather than literal text.
3. New i18n keys carry both arms. The derived key set from task 008 picks them
   up automatically — confirm that, do not re-list them anywhere.
4. Prove non-vacuous, per standing practice: break one variant's name, watch the
   assertion fail naming that variant, restore, confirm no residue. Where an
   assertion has more than one conjunct, prove each independently.

## 8. Documentation · **[Binding]**

None. `docs/src/mvp-acceptance.md` has no Form Mode accessibility item, and this
slice does not add one.

**Do not touch RFC-042 itself.** Recording slice 5 as implemented and moving the
RFC to `rfcs/done/` is my action once this merges.

## 9. Acceptance criteria · **[Binding]**

1. Every block variant is a named `role="group"`.
2. Raw islands state both that they are raw and why, from the existing keys.
3. No literal user-facing strings; both languages.
4. No behaviour change anywhere.
5. Pinned gates green (`+1.88.0`), per task 007 §9.
6. CI green including the WebView regression and the eval-script parse-check.
7. Every file under 500 ELOC. `block_view.rs` is at 266 and this slice adds to
   every variant — if it passes ~450, split it rather than letting it approach
   the gate, as `editor_header.rs` was split in slice 3.
8. **The `0.14.0` tag exists before merge** (§1).

## 10. Prohibited shortcuts · **[Binding]**

- No behaviour change.
- No new island reason text.
- No positional announcements (§5.3).
- No focus movement.
- No merging before the tag.
- No `--force` push.

## 11. Required evidence

- Changed-file list; before/after ELOC for each.
- The per-variant assertion evidence and the §7.4 non-vacuity proof.
- Confirmation that no behaviour changed.
- **Confirmation that the `0.14.0` tag exists**, with the command output.
- Pinned gate output plus the CI run result.
- A manual note if a safe display is available; if not, say so plainly rather
  than omitting it.

## 12. CI and merge

Branch, commit, push, draft PR — pre-authorized. Report the run URL. Merging
requires explicit instruction **and** the §1 tag gate. If `main` has moved,
report the topology and stop.

Commit scope `app:`; reference RFC-042 slice 5.

## 13. Review-request format

`.git-exclude/review-request/<date>-rfc-042-slice-5-form-mode-metadata.md`,
workflow policy §9.2 sections. Lead with the §11 tag confirmation, then the
non-vacuity proof.
