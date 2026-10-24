//! SQLCipher fail-closed connection handling.
//!
//! Invariants:
//!  - `PRAGMA key` is the first statement on every connection.
//!  - `cipher_version` / `cipher_provider` must be present and expected.
//!  - `PRAGMA cipher_status` must be 1 where the SQLCipher build supports it.
//!  - an existing path is never opened with CREATE, so a failed unlock can
//!    never produce a fresh empty database at the ledger path;
//!  - a plaintext SQLite file is rejected before SQLCipher sees it.

use std::path::Path;
use std::time::Duration;

use rusqlite::config::DbConfig;
use rusqlite::{Connection, OpenFlags};

use crate::error::{AppError, AppResult};
use crate::security::crypto::LockedKey;
use crate::security::files::{
    enforce_private_file, file_exists_no_follow, looks_like_plaintext_sqlite,
};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CipherInfo {
    pub version: String,
    pub provider: String,
    pub provider_version: Option<String>,
    pub cipher_status_supported: bool,
    pub memory_security: bool,
}

fn key_sql(dek: &LockedKey) -> String {
    // Only hex characters are interpolated. SQLCipher parses this form as a
    // raw 32-byte key without KDF, so the passphrase never reaches SQLCipher.
    format!("PRAGMA key = \"x'{}'\";", dek.hex())
}

/// Run the fail-closed checks. `cipher_status` was added in SQLCipher 4.12 and
/// is optional for older maintained builds; the keyed read and integrity check
/// remain mandatory.
fn verify_cipher(conn: &Connection) -> AppResult<CipherInfo> {
    use crate::security::cipher_check::{verify_cipher_connection, CipherCheckError};
    let verified = verify_cipher_connection(conn).map_err(|e| match e {
        CipherCheckError::MissingVersion => {
            AppError::CipherUnavailable("cipher_version missing (plain SQLite?)".into())
        }
        CipherCheckError::UnexpectedVersion(v) => {
            AppError::CipherUnavailable(format!("unexpected cipher_version {v}"))
        }
        CipherCheckError::MissingProvider => {
            AppError::CipherUnavailable("cipher_provider missing".into())
        }
        CipherCheckError::StatusNotOne(v) => {
            AppError::CipherUnavailable(format!("cipher_status was {v}, not 1"))
        }
        CipherCheckError::KeyedReadFailed => AppError::Database,
    })?;
    Ok(CipherInfo {
        version: verified.version,
        provider: verified.provider,
        provider_version: verified.provider_version,
        cipher_status_supported: verified.cipher_status_supported,
        memory_security: true,
    })
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA cipher_memory_security = ON;\
         PRAGMA foreign_keys = ON;\
         PRAGMA synchronous = FULL;\
         PRAGMA secure_delete = ON;\
         PRAGMA temp_store = MEMORY;\
         PRAGMA trusted_schema = OFF;\
         PRAGMA recursive_triggers = OFF;",
    )?;
    // WAL must be verified, not assumed.
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(AppError::Database);
    }
    // Defensive mode rejects writing to schema objects from untrusted SQL.
    if conn
        .set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
        .is_err()
    {
        return Err(AppError::Database);
    }
    let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
    if fk != 1 {
        return Err(AppError::Database);
    }
    let trusted: i64 = conn.query_row("PRAGMA trusted_schema", [], |r| r.get(0))?;
    if trusted != 0 {
        return Err(AppError::Database);
    }
    Ok(())
}

/// Open (or create) an encrypted ledger. Never creates at an existing path, and
/// never opens with CREATE unless the caller explicitly asked to create.
pub fn open_ledger(
    path: &Path,
    dek: &LockedKey,
    create: bool,
) -> AppResult<(Connection, CipherInfo)> {
    if create {
        if file_exists_no_follow(path)? {
            return Err(AppError::VaultExists);
        }
        if let Some(parent) = path.parent() {
            crate::security::files::ensure_private_dir(parent)?;
        }
        // Pre-create 0600 so SQLite never creates the ledger with a permissive
        // umask-derived mode. Then open without CREATE.
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| AppError::Io)?
            .sync_all()
            .map_err(|_| AppError::Io)?;
    } else {
        if !file_exists_no_follow(path)? {
            return Err(AppError::VaultMissing);
        }
        enforce_private_file(path)?;
        if looks_like_plaintext_sqlite(path)? {
            return Err(AppError::PlaintextDatabase);
        }
    }

    // Never pass CREATE for the ledger: an existing path must decrypt, and a
    // creation flow pre-created the 0600 file above.
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(|_| AppError::Database)?;

    // Key first. Absolutely nothing may be executed before this.
    conn.execute_batch(&key_sql(dek))
        .map_err(|_| AppError::CipherUnavailable("key rejected".into()))?;

    let info = verify_cipher(&conn)?;
    configure(&conn)?;

    // Enforce 0600 on the ledger and any sidecars the cipher created.
    enforce_private_file(path)?;
    for suffix in ["-wal", "-shm"] {
        let side = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
        if side.exists() {
            enforce_private_file(&side)?;
        }
    }
    if create && looks_like_plaintext_sqlite(path).unwrap_or(false) {
        return Err(AppError::PlaintextDatabase);
    }
    Ok((conn, info))
}

/// Full independent page HMAC verification. Empty result means success.
pub fn cipher_integrity_check(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn
        .prepare("PRAGMA cipher_integrity_check")
        .map_err(|_| AppError::IntegrityFailure)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, Option<String>>(0))
        .map_err(|_| AppError::IntegrityFailure)?;
    let mut errors = Vec::new();
    for value in rows.flatten().flatten() {
        let value = value.trim().to_string();
        if !value.is_empty() && !value.eq_ignore_ascii_case("ok") {
            errors.push(value);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::IntegrityFailure)
    }
}

pub fn sqlite_integrity_check(conn: &Connection) -> AppResult<()> {
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| AppError::IntegrityFailure)?;
    if result.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(AppError::IntegrityFailure)
    }
}

/// Raw-key rekey. Verified in the Phase 0 spike; if this fails on the pinned
/// SQLCipher build the app must stop rather than fall back to passphrase keys.
pub fn rekey_ledger(conn: &Connection, new_dek: &LockedKey) -> AppResult<()> {
    // Re-encrypting under WAL: fold frames into the main database first so the
    // page HMACs verified afterwards describe one consistent file.
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|_| AppError::Database)?;
    conn.execute_batch(&format!("PRAGMA rekey = \"x'{}'\";", new_dek.hex()))
        .map_err(|_| AppError::Database)?;
    // Verify the new key works on this connection and the DB still decrypts.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(|_| AppError::Database)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|_| AppError::Database)?;
    cipher_integrity_check(conn)
}

pub fn cipher_summary(conn: &Connection) -> AppResult<CipherInfo> {
    verify_cipher(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::crypto::LockedKey;

    #[test]
    fn key_sql_contains_only_hex() {
        let key = LockedKey::new([0xab; 32]);
        let sql = key_sql(&key);
        assert!(sql.starts_with("PRAGMA key = \"x'"));
        assert_eq!(sql.matches('\'').count(), 2);
        assert_eq!(sql.matches('"').count(), 2);
        assert!(sql.chars().all(|c| c.is_ascii() && !c.is_control()));
    }
}
