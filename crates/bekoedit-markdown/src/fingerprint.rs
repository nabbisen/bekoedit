//! Revision-scoped block identity and fingerprints (RFC-014).
//!
//! `BlockId` values are regenerated on every full reparse. The UI must not
//! assume they are stable across revisions; the Rust core validates ordinal,
//! kind, and fingerprint before resolving any byte range from them.

use serde::{Deserialize, Serialize};

use crate::block::BlockKind;

/// Stable, dependency-free 64-bit FNV-1a hash.
///
/// Fingerprints are runtime-only validation data (never persisted), so a
/// small deterministic hash is sufficient and keeps the crate light.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Content + context fingerprint of a block (RFC-014 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockFingerprint {
    /// Hash of the block's exact source slice.
    pub content_hash: u64,
    /// Hash of up to 32 bytes preceding the block (context anchor).
    pub prefix_context_hash: u64,
    /// Hash of up to 32 bytes following the block (context anchor).
    pub suffix_context_hash: u64,
}

impl BlockFingerprint {
    pub fn compute(text: &str, start: usize, end: usize) -> Self {
        let content_hash = fnv1a(&text.as_bytes()[start..end]);
        let prefix_start = floor_char_boundary(text, start.saturating_sub(32));
        let suffix_end = ceil_char_boundary(text, (end + 32).min(text.len()));
        Self {
            content_hash,
            prefix_context_hash: fnv1a(&text.as_bytes()[prefix_start..start]),
            suffix_context_hash: fnv1a(&text.as_bytes()[end..suffix_end]),
        }
    }
}

fn floor_char_boundary(text: &str, mut i: usize) -> usize {
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(text: &str, mut i: usize) -> usize {
    while i < text.len() && !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Revision-scoped logical block identity (RFC-014 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId {
    /// Document revision at which this id was generated.
    pub revision_created: u64,
    /// Zero-based position among top-level blocks at that revision.
    pub ordinal: u32,
    pub kind: BlockKind,
    pub fingerprint: BlockFingerprint,
}
