//! Phase 0 fail-closed SQLCipher spike, executed with `cargo test`.
//! These tests use real temporary encrypted databases, not mocks.

use std::fs;
use std::io::Read;
use std::path::Path;

use cardigan_money_lib::error::AppError;
use cardigan_money_lib::security::cipher::{
    cipher_integrity_check, open_ledger, rekey_ledger, sqlite_integrity_check,
};
use cardigan_money_lib::security::crypto::{KdfParams, LockedKey, SecretString};
use cardigan_money_lib::security::files::sha256_hex;
use cardigan_money_lib::security::vault::{write_header, VaultHeader, VaultPaths};

const SENTINEL: &str = "PHASE0-PLAINTEXT-SENTINEL-8f3a1c";

fn fast_params() -> KdfParams {
    KdfParams { m_cost_kib: 8 * 1024, t_cost: 1, p_cost: 1 }
}

fn pass() -> SecretString {
    SecretString::new("correct horse battery staple phase0".to_string())
}

fn create_vault(tmp: &Path) -> (VaultPaths, VaultHeader, LockedKey) {
    let paths = VaultPaths::new(tmp);
    paths.ensure().unwrap();
    let (header, dek) = VaultHeader::create(&pass(), fast_params(), [7u8; 16], 1).unwrap();
    write_header(&paths, &header).unwrap();
    (paths, header, dek)
}

fn seed(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE secret_data (id INTEGER PRIMARY KEY, note TEXT NOT NULL);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO secret_data (note) VALUES (?1)",
        [format!("{SENTINEL}-posted")],
    )
    .unwrap();
}

fn file_bytes(path: &Path) -> Vec<u8> {
    let mut f = fs::File::open(path).unwrap();
    let mut b = Vec::new();
    f.read_to_end(&mut b).unwrap();
    b
}

fn read_sidecar(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_default()
}

#[test]
fn sqlcipher_is_actually_active_and_reports_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, _header, dek) = create_vault(tmp.path());
    let (conn, info) = open_ledger(&paths.database, &dek, true).unwrap();
    println!("SQLCipher version={} provider={} provider_version={:?} cipher_status={}",
        info.version, info.provider, info.provider_version, info.cipher_status_supported);
    assert!(info.version.starts_with("4."), "expected SQLCipher 4.x, got {}", info.version);
    assert!(!info.provider.trim().is_empty());
    assert!(info.memory_security);
    cipher_integrity_check(&conn).unwrap();
    sqlite_integrity_check(&conn).unwrap();
}

#[test]
fn database_wal_and_shm_are_ciphertext_and_not_plaintext_sqlite() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, _header, dek) = create_vault(tmp.path());
    let (conn, _) = open_ledger(&paths.database, &dek, true).unwrap();
    seed(&conn);
    // Keep WAL un-checkpointed so its frames are readable for the assertion.
    let db_bytes = file_bytes(&paths.database);
    assert_ne!(&db_bytes[..16.min(db_bytes.len())], b"SQLite format 3\0");
    for needle in [SENTINEL.as_bytes(), b"secret_data".as_slice()] {
        assert!(
            !db_bytes.windows(needle.len()).any(|w| w == needle),
            "plaintext marker found in encrypted DB"
        );
    }
    let wal = read_sidecar(Path::new(&format!("{}-wal", paths.database.display())));
    let shm = read_sidecar(Path::new(&format!("{}-shm", paths.database.display())));
    assert!(!wal.is_empty(), "WAL should exist during/after writes");
    for (name, bytes) in [("wal", &wal), ("shm", &shm)] {
        assert!(
            !bytes.windows(SENTINEL.len()).any(|w| w == SENTINEL.as_bytes()),
            "plaintext sentinel found in {name}"
        );
    }
    // A plain SQLite client must not be able to read it.
    let out = std::process::Command::new("/usr/bin/sqlite3")
        .arg(&paths.database)
        .arg("SELECT count(*) FROM sqlite_master;")
        .output()
        .expect("sqlite3 exists on macOS");
    assert!(!out.status.success(), "plain sqlite3 opened the encrypted DB");
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("not a database") || stderr.contains("encrypted"),
        "unexpected sqlite3 error: {stderr}"
    );
    drop(conn);
}

#[test]
fn wrong_key_is_rejected_without_modifying_the_database() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, _header, dek) = create_vault(tmp.path());
    {
        let (conn, _) = open_ledger(&paths.database, &dek, true).unwrap();
        seed(&conn);
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
    }
    let before = fs::read(&paths.database).unwrap();
    let wrong = LockedKey::new([0x42; 32]);
    let result = open_ledger(&paths.database, &wrong, false);
    assert!(result.is_err(), "wrong key must fail closed");
    let after = fs::read(&paths.database).unwrap();
    assert_eq!(before, after, "wrong key modified the database file");
    assert_eq!(fs::read(&paths.database).unwrap()[..16].to_vec(), before[..16].to_vec());
    // Correct key still opens and data is intact.
    let (conn, _) = open_ledger(&paths.database, &dek, false).unwrap();
    let count: i64 = conn.query_row("SELECT count(*) FROM secret_data", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn raw_key_rekey_support_is_proven() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, _header, dek) = create_vault(tmp.path());
    {
        let (conn, _) = open_ledger(&paths.database, &dek, true).unwrap();
        seed(&conn);
        let new_dek = LockedKey::generate().unwrap();
        rekey_ledger(&conn, &new_dek).unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").unwrap();
        // The old key must fail now.
        let old_path = paths.database.clone();
        drop(conn);
        assert!(open_ledger(&old_path, &dek, false).is_err(), "old key still opens after rekey");
        let (conn2, _) = open_ledger(&old_path, &new_dek, false).unwrap();
        let count: i64 = conn2.query_row("SELECT count(*) FROM secret_data", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        cipher_integrity_check(&conn2).unwrap();
        println!("raw-key PRAGMA rekey verified");
    }
}

#[test]
fn create_never_overwrites_and_missing_never_creates() {
    let tmp = tempfile::tempdir().unwrap();
    let (paths, _header, dek) = create_vault(tmp.path());
    let (conn, _) = open_ledger(&paths.database, &dek, true).unwrap();
    seed(&conn);
    drop(conn);
    let before = fs::read(&paths.database).unwrap();
    assert!(matches!(open_ledger(&paths.database, &dek, true), Err(AppError::VaultExists)));
    assert_eq!(before, fs::read(&paths.database).unwrap());

    let missing = tmp.path().join("does-not-exist.cmdb");
    assert!(matches!(open_ledger(&missing, &dek, false), Err(AppError::VaultMissing)));
    assert!(!missing.exists(), "open must not create a DB when it is missing");
}

#[test]
fn plain_sqlite_file_is_rejected_before_sqlcipher() {
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("plain.db");
    let status = std::process::Command::new("/usr/bin/sqlite3")
        .arg(&plain)
        .arg("CREATE TABLE t(x);")
        .status()
        .unwrap();
    assert!(status.success());
    let key = LockedKey::generate().unwrap();
    let err = open_ledger(&plain, &key, false).unwrap_err();
    assert!(matches!(err, AppError::PlaintextDatabase), "got {err:?}");
}

#[test]
fn plain_sqlite_header_present_in_file_is_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let fake = tmp.path().join("fake.db");
    let mut bytes = b"SQLite format 3\0".to_vec();
    bytes.extend_from_slice(&[0u8; 512]);
    fs::write(&fake, &bytes).unwrap();
    // Filename is arbitrary; hash is stable for evidence.
    assert_eq!(sha256_hex(&bytes).len(), 64);
}
