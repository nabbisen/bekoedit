# Saving, Recovery, and Conflicts

## Autosave

Edits schedule a debounced autosave (about 1.5s after you stop). The
status bar shows the lifecycle: unsaved changes → autosave pending →
saving → all changes saved. Manual save is always available.

## Atomic writes

Saves write a temporary file next to the target, flush it, then rename it
over the original — a crash mid-save can't leave a half-written file.

## Recovery snapshots

While a document is dirty, bekoedit keeps a snapshot in your local app
data directory (outside the workspace). It is removed after a confirmed
save. Snapshots are never automatically written over your files.

## External changes

Before every write, bekoedit compares the file on disk with its
last-known fingerprint. If another program changed (or deleted) the file
while you have unsaved edits, autosave pauses and a banner asks you to
choose:

- **Keep my version** — write your in-memory text over the disk version.
- **Reload from disk** — discard your local edits.
- **Save my version as a copy** — keep both: disk stays as-is, your text
  goes to a new file.

Neither version is ever lost silently.

## Failures

If a save fails (permissions, disk full…), your text stays intact in
memory, the status bar shows the failure, and you can retry.
