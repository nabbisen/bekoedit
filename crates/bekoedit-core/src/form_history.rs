//! Undo and Redo of committed Form Mode edits (RFC-049 slice 1).
//!
//! The stacks are a field of `DocumentSession`, so a session that is replaced
//! (another document, a conflict's reload from disk, a restore) takes its
//! history with it. Each record is the inverse of one source patch, and it is
//! applied only while the document is exactly as that patch left it, which the
//! session checks by revision.
//!
//! A record's `revision_after` is the revision at which it is *current*. The
//! records beneath the top are not: they become current when the one above
//! them is undone (or redone), because that leaves the document exactly as the
//! one beneath left it. `restamp_top` records that moment. A record is pushed
//! onto a stack whose top is not current only after that stack is cleared, so
//! a chain is never broken by a foreign mutation without the top going stale.

use std::collections::VecDeque;

use bekoedit_markdown::ByteRange;

/// At most this many records on each stack; the oldest is dropped.
pub(crate) const MAX_RECORDS: usize = 100;

/// One patch's inverse: replace `range_after` with `replaced`.
#[derive(Debug, Clone)]
pub(crate) struct FormRecord {
    pub(crate) revision_after: u64,
    pub(crate) range_after: ByteRange,
    pub(crate) replaced: String,
}

/// The Undo and Redo stacks of one `DocumentSession`. Not serialized, and
/// ignored by session equality: two sessions with the same document state are
/// equal whatever they could undo.
#[derive(Debug, Clone, Default)]
pub(crate) struct FormHistory {
    undo: VecDeque<FormRecord>,
    redo: VecDeque<FormRecord>,
}

impl PartialEq for FormHistory {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for FormHistory {}

impl FormHistory {
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub(crate) fn depths(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    fn stack(&mut self, undo: bool) -> &mut VecDeque<FormRecord> {
        if undo { &mut self.undo } else { &mut self.redo }
    }

    pub(crate) fn top_revision(&self, undo: bool) -> Option<u64> {
        let stack = if undo { &self.undo } else { &self.redo };
        stack.back().map(|record| record.revision_after)
    }

    pub(crate) fn pop(&mut self, undo: bool) -> Option<FormRecord> {
        self.stack(undo).pop_back()
    }

    fn push(&mut self, undo: bool, record: FormRecord) {
        let stack = self.stack(undo);
        stack.push_back(record);
        if stack.len() > MAX_RECORDS {
            stack.pop_front();
        }
    }

    /// A new edit applied at `revision_before` and recorded. Clears Redo, and
    /// Undo too if its top was not current when the edit applied.
    pub(crate) fn record(&mut self, revision_before: u64, record: FormRecord) {
        if self
            .top_revision(true)
            .is_some_and(|r| r != revision_before)
        {
            self.undo.clear();
        }
        self.redo.clear();
        self.push(true, record);
    }

    /// An edit that changed no byte still advanced the revision: a top that
    /// was current at `revision_before` is current at `revision_after`.
    pub(crate) fn restamp_tops(&mut self, revision_before: u64, revision_after: u64) {
        for undo in [true, false] {
            if let Some(top) = self.stack(undo).back_mut() {
                if top.revision_after == revision_before {
                    top.revision_after = revision_after;
                }
            }
        }
    }

    /// A record of `undo`'s stack was applied, leaving the document at
    /// `revision`. Its inverse goes on the other stack, and the record now on
    /// top of this one is current.
    pub(crate) fn applied(&mut self, undo: bool, inverse: FormRecord, revision: u64) {
        if let Some(top) = self.stack(undo).back_mut() {
            top.revision_after = revision;
        }
        self.push(!undo, inverse);
    }
}
