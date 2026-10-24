//! Build-agnostic SQLCipher verification.
//!
//! This file is compiled both into the real application (which links SQLCipher)
//! and into `tools/plain-sqlite-probe` (which deliberately links plain SQLite).
//! The plain build proves that this exact check refuses to operate without
//! SQLCipher. Keep this module free of application-specific types.

use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct VerifiedCipher {
    pub version: String,
    pub provider: String,
    pub provider_version: Option<String>,
    pub cipher_status_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CipherCheckError {
    MissingVersion,
    UnexpectedVersion(String),
    MissingProvider,
    StatusNotOne(i64),
    KeyedReadFailed,
}

fn pragma_string(conn: &Connection, pragma: &str) -> Option<String> {
    conn.query_row(pragma, [], |row| row.get::<_, Option<String>>(0))
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
}

pub fn verify_cipher_connection(conn: &Connection) -> Result<VerifiedCipher, CipherCheckError> {
    let version =
        pragma_string(conn, "PRAGMA cipher_version").ok_or(CipherCheckError::MissingVersion)?;
    if !version.starts_with("4.") {
        return Err(CipherCheckError::UnexpectedVersion(version));
    }
    let provider =
        pragma_string(conn, "PRAGMA cipher_provider").ok_or(CipherCheckError::MissingProvider)?;
    let provider_version = pragma_string(conn, "PRAGMA cipher_provider_version");

    let mut cipher_status_supported = false;
    match conn.query_row("PRAGMA cipher_status", [], |row| row.get::<_, i64>(0)) {
        Ok(1) => cipher_status_supported = true,
        Ok(other) => return Err(CipherCheckError::StatusNotOne(other)),
        // SQLCipher versions before 4.12 do not implement cipher_status.
        Err(_) => {}
    }

    // Keyed read: on plain SQLite this may succeed (the file is plaintext), but
    // the version check above already failed. On SQLCipher it proves the key
    // was accepted.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(|_| CipherCheckError::KeyedReadFailed)?;

    Ok(VerifiedCipher {
        version,
        provider,
        provider_version,
        cipher_status_supported,
    })
}
