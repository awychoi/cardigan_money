//! Filesystem discipline for the vault: 0700 directories, 0600 files, no
//! symlinks, atomic replace with fsync, and bounded reads.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::security::crypto::random_array;

pub const MAX_HEADER_BYTES: u64 = 4096;

fn current_uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn is_symlink(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink()
}

/// Create/verify a directory only the owner can access (0700).
pub fn ensure_private_dir(dir: &Path) -> AppResult<()> {
    match fs::symlink_metadata(dir) {
        Ok(meta) => {
            if is_symlink(&meta) || !meta.is_dir() {
                return Err(AppError::PermissionDenied("unsafe vault directory"));
            }
            if meta.uid() != current_uid() {
                return Err(AppError::PermissionDenied("vault directory owner mismatch"));
            }
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        }
        Err(_) => return Err(AppError::Io),
    }
    let meta = fs::symlink_metadata(dir)?;
    if is_symlink(&meta) || !meta.is_dir() || meta.mode() & 0o077 != 0 {
        return Err(AppError::PermissionDenied("vault directory permissions"));
    }
    Ok(())
}

/// Verify an existing regular file and force 0600. Rejects symlinks outright.
pub fn enforce_private_file(path: &Path) -> AppResult<()> {
    let meta = fs::symlink_metadata(path)?;
    if is_symlink(&meta) {
        return Err(AppError::PermissionDenied("symlink rejected"));
    }
    if !meta.is_file() {
        return Err(AppError::PermissionDenied("not a regular file"));
    }
    if meta.uid() != current_uid() {
        return Err(AppError::PermissionDenied("file owner mismatch"));
    }
    if meta.mode() & 0o077 != 0 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn file_exists_no_follow(path: &Path) -> AppResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if is_symlink(&meta) => Err(AppError::PermissionDenied("symlink rejected")),
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(AppError::Io),
    }
}

pub fn read_private_file(path: &Path, max_bytes: u64) -> AppResult<Vec<u8>> {
    enforce_private_file(path)?;
    let meta = fs::symlink_metadata(path)?;
    if meta.len() == 0 || meta.len() > max_bytes {
        return Err(AppError::HeaderInvalid("unexpected file size"));
    }
    let file = OpenOptions::new().read(true).open(path)?;
    let mut buf = Vec::with_capacity(meta.len() as usize);
    file.take(max_bytes).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Atomic write: unique 0600 temp file in the destination directory, fsync,
/// rename, then fsync the directory. A failed write leaves the old file intact.
pub fn atomic_write_private(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path.parent().ok_or(AppError::Io)?;
    ensure_private_dir(parent)?;
    if fs::symlink_metadata(path)
        .map(|m| is_symlink(&m))
        .unwrap_or(false)
    {
        return Err(AppError::PermissionDenied("symlink rejected"));
    }
    let suffix: [u8; 8] = random_array()?;
    let temp: PathBuf = parent.join(format!(
        ".cm-tmp-{}-{}",
        std::process::id(),
        hex::encode(suffix)
    ));
    let result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        fsync_dir(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub fn fsync_dir(dir: &Path) -> AppResult<()> {
    let d = File::open(dir)?;
    d.sync_all()?;
    Ok(())
}

pub fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if is_symlink(&meta) => Err(AppError::PermissionDenied("symlink rejected")),
        Ok(_) => {
            fs::remove_file(path)?;
            if let Some(parent) = path.parent() {
                fsync_dir(parent)?;
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AppError::Io),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// True when the file begins with the unencrypted SQLite header.
pub fn looks_like_plaintext_sqlite(path: &Path) -> AppResult<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 16];
    if f.read(&mut magic)? < 16 {
        return Ok(false);
    }
    Ok(&magic == b"SQLite format 3\0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_sets_0600_and_replaces() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("vault");
        ensure_private_dir(&dir).unwrap();
        let file = dir.join("vault.header");
        atomic_write_private(&file, b"first").unwrap();
        atomic_write_private(&file, b"second").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"second");
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // no temp litter
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn symlink_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("vault");
        ensure_private_dir(&dir).unwrap();
        let real = tmp.path().join("outside");
        fs::write(&real, b"x").unwrap();
        let link = dir.join("vault.header");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(matches!(
            read_private_file(&link, MAX_HEADER_BYTES),
            Err(AppError::PermissionDenied(_))
        ));
    }

    #[test]
    fn plaintext_sqlite_detection() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("plain.db");
        fs::write(&f, b"SQLite format 3\0rest").unwrap();
        assert!(looks_like_plaintext_sqlite(&f).unwrap());
        let g = tmp.path().join("cipher.db");
        fs::write(&g, b"\x01\x02\x03not sqlite").unwrap();
        assert!(!looks_like_plaintext_sqlite(&g).unwrap());
    }
}
