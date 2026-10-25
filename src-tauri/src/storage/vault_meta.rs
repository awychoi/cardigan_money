use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::security::cipher::CipherInfo;
use crate::security::vault::VaultHeader;
use crate::storage::time::now_rfc3339;

#[derive(Debug, Clone)]
pub struct VaultMeta {
    pub vault_id: String,
    pub cipher_version: String,
    pub cipher_provider: String,
    pub schema_version: u32,
    pub generation: u32,
    pub created_at: String,
    pub rotated_at: Option<String>,
    pub last_integrity_check_at: Option<String>,
}

pub fn insert_initial(
    conn: &Connection,
    header: &VaultHeader,
    cipher: &CipherInfo,
    schema_version: u32,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO vault_metadata
         (id, vault_id, cipher_version, cipher_provider, schema_version, generation, created_at, rotated_at, last_integrity_check_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, NULL, ?6)",
        rusqlite::params![
            header.vault_id_hex(),
            cipher.version,
            cipher.provider,
            schema_version,
            header.generation,
            now_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn load(conn: &Connection) -> AppResult<VaultMeta> {
    conn.query_row(
        "SELECT vault_id, cipher_version, cipher_provider, schema_version, generation,
                created_at, rotated_at, last_integrity_check_at
         FROM vault_metadata WHERE id = 1",
        [],
        |row| {
            Ok(VaultMeta {
                vault_id: row.get(0)?,
                cipher_version: row.get(1)?,
                cipher_provider: row.get(2)?,
                schema_version: row.get(3)?,
                generation: row.get(4)?,
                created_at: row.get(5)?,
                rotated_at: row.get(6)?,
                last_integrity_check_at: row.get(7)?,
            })
        },
    )
    .map_err(|_| AppError::Database)
}

pub fn set_integrity_checked(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE vault_metadata SET last_integrity_check_at = ?1 WHERE id = 1",
        [now_rfc3339()],
    )?;
    Ok(())
}

pub fn set_generation(conn: &Connection, generation: u32, rotated: bool) -> AppResult<()> {
    if rotated {
        conn.execute(
            "UPDATE vault_metadata SET generation = ?1, rotated_at = ?2, last_integrity_check_at = ?2 WHERE id = 1",
            rusqlite::params![generation, now_rfc3339()],
        )?;
    } else {
        conn.execute(
            "UPDATE vault_metadata SET generation = ?1 WHERE id = 1",
            [generation],
        )?;
    }
    Ok(())
}

pub fn set_schema_version(conn: &Connection, version: u32) -> AppResult<()> {
    conn.execute(
        "UPDATE vault_metadata SET schema_version = ?1 WHERE id = 1",
        [version],
    )?;
    Ok(())
}

fn unix_to_rfc3339(value: i64) -> Option<String> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(value, 0)
        .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Reconcile the restored database's public metadata with the header that was
/// or will be installed. Used by same-vault restore (keep current identity) and
/// by cross-vault/fresh restore (adopt the backup identity with a new DEK).
pub fn reconcile_identity(
    conn: &Connection,
    header: &VaultHeader,
    cipher: &CipherInfo,
    schema_version: u32,
) -> AppResult<()> {
    let created = unix_to_rfc3339(header.created_at).unwrap_or_else(now_rfc3339);
    let rotated = if header.rotated_at > 0 {
        unix_to_rfc3339(header.rotated_at)
    } else {
        None
    };
    conn.execute(
        "UPDATE vault_metadata SET vault_id = ?1, cipher_version = ?2, cipher_provider = ?3, \
         schema_version = ?4, generation = ?5, created_at = ?6, rotated_at = ?7, \
         last_integrity_check_at = ?8 WHERE id = 1",
        rusqlite::params![
            header.vault_id_hex(),
            cipher.version,
            cipher.provider,
            schema_version,
            header.generation,
            created,
            rotated,
            now_rfc3339()
        ],
    )?;
    Ok(())
}

/// Insert demo-mode audit metadata (no vault header; fixed synthetic key).
pub fn insert_demo_metadata(
    conn: &Connection,
    cipher: &CipherInfo,
    schema_version: u32,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO vault_metadata
         (id, vault_id, cipher_version, cipher_provider, schema_version, generation, created_at, rotated_at, last_integrity_check_at)
         VALUES (1, ?1, ?2, ?3, ?4, 1, ?5, NULL, ?5)",
        rusqlite::params![
            crate::demo::fixtures::DEMO_VAULT_ID,
            cipher.version,
            cipher.provider,
            schema_version,
            now_rfc3339()
        ],
    )?;
    Ok(())
}
