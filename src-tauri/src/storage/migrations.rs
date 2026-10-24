//! Forward-only migrations with checksum verification and a pre-migration
//! encrypted snapshot. A failed migration rolls back its transaction and
//! leaves the original database intact.

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::security::crypto::LockedKey;
use crate::storage::snapshot::create_encrypted_snapshot;
use crate::storage::time::now_rfc3339;

pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_metadata_settings_events",
        sql: include_str!("../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "domain_accounts_transactions",
        sql: include_str!("../../migrations/0002_domain.sql"),
    },
    Migration {
        version: 3,
        name: "edits_rules_imports",
        sql: include_str!("../../migrations/0003_edits_rules_imports.sql"),
    },
];

pub fn supported_schema_version() -> u32 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

fn checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_migration_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            checksum TEXT NOT NULL,
            applied_at TEXT NOT NULL
        ) STRICT;",
    )?;
    Ok(())
}

fn applied(conn: &Connection) -> AppResult<Vec<(u32, String)>> {
    let mut stmt =
        conn.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

/// Apply pending migrations. `dek` is needed for the pre-migration encrypted
/// snapshot; `snapshot_dir` is the vault directory.
pub fn run_migrations(
    conn: &Connection,
    dek: &LockedKey,
    snapshot_dir: &std::path::Path,
) -> AppResult<u32> {
    ensure_migration_table(conn)?;
    let already = applied(conn)?;
    let supported = supported_schema_version();

    for (version, stored_checksum) in &already {
        let Some(known) = MIGRATIONS.iter().find(|m| m.version == *version) else {
            return Err(AppError::SchemaTooNew);
        };
        if &checksum(known.sql) != stored_checksum {
            // An applied migration was edited. Fail closed rather than operate
            // on a schema whose provenance we cannot trust.
            return Err(AppError::Database);
        }
    }

    let pending: Vec<&Migration> = MIGRATIONS
        .iter()
        .filter(|m| !already.iter().any(|(v, _)| *v == m.version))
        .collect();
    if pending.is_empty() {
        return Ok(already.iter().map(|(v, _)| *v).max().unwrap_or(0));
    }

    // Snapshot before changing a non-empty ledger. An initial empty database
    // does not need a snapshot.
    let table_count: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'schema_migrations'",
        [],
        |row| row.get(0),
    )?;
    if table_count > 0 {
        let target = pending.last().map(|m| m.version).unwrap_or(supported);
        let stamp = now_rfc3339().replace([':', '-'], "");
        let path = snapshot_dir.join(format!("pre-migration-v{target}-{stamp}.cmdb"));
        create_encrypted_snapshot(conn, &path, dek)?;
    }

    for migration in pending {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                migration.version,
                migration.name,
                checksum(migration.sql),
                now_rfc3339()
            ],
        )?;
        tx.commit()?;
    }

    Ok(applied(conn)?.iter().map(|(v, _)| *v).max().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_version_matches_latest_migration() {
        assert_eq!(supported_schema_version(), 3);
        assert_eq!(MIGRATIONS[0].version, 1);
        assert_eq!(MIGRATIONS[1].version, 2);
        assert_eq!(MIGRATIONS[2].version, 3);
        assert!(!checksum(MIGRATIONS[2].sql).is_empty());
    }
}
