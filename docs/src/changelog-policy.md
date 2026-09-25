# Changelog and Release Notes

`CHANGELOG.md` is the one place where release notes live. Nothing is written
twice: each release has one section, in one file, and the GitHub Release page
only links to it.

## Each release's section

Each version's section opens with a short **Highlights** list, followed by the
detail:

```markdown
## [0.16.1] - 2026-09-26

### Highlights
- One line per change a user will notice, in the user's words.
- Security fixes are named plainly.

### Added
### Changed
### Fixed
### Security
```

- **Highlights are short.** About 3 to 7 bullets, one or two lines each. They
  say what changed for the person using bekoedit, not which task, RFC or CI job
  did it.
- **The detail sections** follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
  as the rest of the file does. List only the sections that have something in
  them.
- **Work lands under `[Unreleased]`** as it merges. At release, the heading is
  renamed to the version and dated with the release day, and the Highlights
  list is written above the detail.

## The GitHub Release page

The page carries **one line**, a link to `CHANGELOG.md` **at the release's own
tag**. At its tag, a version is always the first section of `CHANGELOG.md`.

The link is pinned to the tag, not to `main`, so it still works when older
sections are later moved (next section). Nothing else is written on the page.

## Keeping the file a readable size

`CHANGELOG.md` holds **the current series only**. Older series are **moved**,
never copied, into one file each under `changelog/`, and `CHANGELOG.md` ends
with links to them.

- **A series, before 1.0.0, is ten minor versions and their patches:** 0.1–0.9,
  0.10–0.19, 0.20–0.29, and so on. The series ends when the next one's first
  release ships. For example, when 0.20.0 is released, 0.10–0.19 moves to
  `changelog/0.10-0.19.md`.
- **From 1.0.0, a series is one major version**: 1.x, then 2.x, and so on. When
  2.0.0 ships, 1.x moves to `changelog/1.x.md`.
- **An archive file is written once**, when its series ends, and is not edited
  afterwards.
- **Each version appears in exactly one file.**

**Known limit:** a link that someone outside the project made to a section of
`CHANGELOG.md` on `main` stops working when that series moves. Links pinned to a
tag, including every GitHub Release link, are not affected.

## What checks this

| Rule | Checked by | Status |
|---|---|---|
| The released version has a dated section | `release.yml`, before anything is built | in place |
| That section opens with `### Highlights` | `release.yml`, same step | in place |
| The GitHub Release page links to `CHANGELOG.md` at the tag | `release.yml` sets the page text | in place |
| Each version appears exactly once across `CHANGELOG.md` and `changelog/` | `scripts/check-changelog.sh`, in CI | in place |
| Every archive file is linked from `CHANGELOG.md`, and every link resolves | same check | in place |
| No file in the repository links to a section that has moved | same check | in place |
| `CHANGELOG.md` holds only the current series | same check | in place |

The check also names an archive file that is misnamed or holds another series,
and a version's link definition left behind in the other file. Its own tests
(`scripts/test-check-changelog.sh`) run in CI too, and break each rule on a
throwaway tree to show it fails by name.
