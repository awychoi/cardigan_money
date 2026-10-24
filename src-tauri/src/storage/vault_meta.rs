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
