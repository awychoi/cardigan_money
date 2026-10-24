// Test-only helper for scripts/verify-plain-sqlite-reject.sh. Creates an
// encrypted spike database at the given path with synthetic content.
use cardigan_money_lib::security::cipher::open_ledger;
use cardigan_money_lib::security::crypto::{KdfParams, SecretString};
use cardigan_money_lib::security::vault::VaultHeader;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: make_spike_db <path>");
    let path = std::path::PathBuf::from(path);
    if path.exists() {
        std::fs::remove_file(&path).unwrap();
    }
    let pass = SecretString::new("spike-only passphrase with enough chars".into());
    let (_header, dek) = VaultHeader::create(
        &pass,
        KdfParams {
            m_cost_kib: 8192,
            t_cost: 1,
            p_cost: 1,
        },
        [9u8; 16],
        1,
    )
    .unwrap();
    let (conn, info) = open_ledger(&path, &dek, true).unwrap();
    conn.execute_batch(
        "CREATE TABLE spike (id INTEGER PRIMARY KEY, note TEXT NOT NULL);
         INSERT INTO spike(note) VALUES ('synthetic spike row');",
    )
    .unwrap();
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    println!("cipher={} provider={}", info.version, info.provider);
    println!("created {}", path.display());
}
