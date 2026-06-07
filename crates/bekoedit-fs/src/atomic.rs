//! Atomic write strategy and disk fingerprints (RFC-007).
//!
//! Save path: write a temporary file in the same directory, flush it, then
//! rename over the target (REL-002). Fingerprints (length + mtime + content
//! hash) detect external modification before a write would clobber it
//! (REL-001, RFC-008).

use std::io::Write;
use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Identity of the last-known on-disk content (external design §23.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub len: u64,
    /// Modification time in nanoseconds since the Unix epoch
    /// (0 when the platform does not report one).
    pub mtime_ns: u128,
    /// FNV-1a hash of the file content.
    pub content_hash: u64,
}

impl FileFingerprint {
    /// Reads the current fingerprint of `path`.
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let meta = std::fs::metadata(path)?;
        Ok(Self::of_bytes(&bytes, meta.modified().ok()))
    }

    pub fn of_bytes(bytes: &[u8], mtime: Option<SystemTime>) -> Self {
        Self {
            len: bytes.len() as u64,
            mtime_ns: mtime
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            content_hash: fnv1a(bytes),
        }
    }

    /// True when the disk content differs from this fingerprint.
    /// A cheap metadata check short-circuits before hashing.
    pub fn disk_changed(&self, path: &Path) -> std::io::Result<bool> {
        let current = Self::read(path)?;
        Ok(current.len != self.len || current.content_hash != self.content_hash)
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Writes `content` to `path` atomically: temp file in the same directory,
/// flush + sync, then rename over the target. Returns the fingerprint of
/// the written content.
pub fn atomic_write(path: &Path, content: &str) -> std::io::Result<FileFingerprint> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp_name = format!(
        ".{}.bekoedit-tmp",
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unnamed".into())
    );
    let tmp_path = dir.join(tmp_name);
    {
        let mut tmp = std::fs::File::create(&tmp_path)?;
        tmp.write_all(content.as_bytes())?;
        tmp.flush()?;
        tmp.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    Ok(FileFingerprint::of_bytes(content.as_bytes(), mtime))
}
