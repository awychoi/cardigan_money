//! Consistent encrypted database snapshots via SQLite's backup API.
//!
//! The snapshot is encrypted with the same DEK as the live ledger and carries
//! no plaintext. WAL is checkpointed first. This primitive is used for
//! pre-migration and pre-rekey safety; Phase 7 adds the self-contained backup
//! container with the wrapped-DEK header.

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::security::cipher::{cipher_integrity_check, open_ledger, sqlite_integrity_check};
use crate::security::crypto::LockedKey;
use crate::security::files::{enforce_private_file, file_exists_no_follow};

pub fn create_encrypted_snapshot(
    source: &Connection,
    destination: &Path,
    dek: &LockedKey,
) -> AppResult<()> {
    if file_exists_no_follow(destination)? {
        return Err(AppError::VaultExists);
    }
    if let Some(parent) = destination.parent() {
        crate::security::files::ensure_private_dir(parent)?;
    }
    source
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|_| AppError::Database)?;

    // Pre-create the destination as 0600 so SQLite never creates it with a
    // permissive umask-derived mode.
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?
        .sync_all()?;

    let result = (|| -> AppResult<()> {
        let (mut target, _) = open_ledger(destination, dek, false)?;
        {
            let backup = rusqlite::backup::Backup::new(source, &mut target)
                .map_err(|_| AppError::Database)?;
            backup
                .run_to_completion(256, Duration::from_millis(1), None)
                .map_err(|_| AppError::Database)?;
        }
        target
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|_| AppError::Database)?;
        drop(target);

        enforce_private_file(destination)?;
        // Reopen the finished snapshot read/write and verify it.
        let (verify, _) = open_ledger(destination, dek, false)?;
        cipher_integrity_check(&verify)?;
        sqlite_integrity_check(&verify)?;
        verify
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|_| AppError::Database)?;
        drop(verify);
        // No stale sidecars next to the snapshot.
        for suffix in ["-wal", "-shm"] {
            let side = std::path::PathBuf::from(format!("{}{suffix}", destination.display()));
            let _ = std::fs::remove_file(side);
        }
        let parent = destination.parent().ok_or(AppError::Io)?;
        crate::security::files::fsync_dir(parent)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(destination);
        let _ = std::fs::remove_file(format!("{}-wal", destination.display()));
        let _ = std::fs::remove_file(format!("{}-shm", destination.display()));
    }
    result
}
