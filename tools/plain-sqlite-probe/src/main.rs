#[path = "../../../src-tauri/src/security/cipher_check.rs"]
mod cipher_check;

use rusqlite::Connection;

fn main() {
    let path = std::env::args().nth(1).expect("usage: plain-sqlite-probe <encrypted-db>");
    let conn = Connection::open(&path).expect("open db");
    // A plain SQLite build accepts the unknown PRAGMA key as a no-op.
    let _ = conn.execute_batch("PRAGMA key = \"x'0000000000000000000000000000000000000000000000000000000000000000'\";");
    match cipher_check::verify_cipher_connection(&conn) {
        Ok(info) => {
            eprintln!(
                "FAIL: plain-SQLite build was accepted as SQLCipher (version={} provider={})",
                info.version, info.provider
            );
            std::process::exit(1);
        }
        Err(err) => {
            println!("PASS: plain-SQLite build rejected by the exact app check: {err:?}");
            std::process::exit(2);
        }
    }
}
