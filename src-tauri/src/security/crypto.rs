//! Cryptographic primitives for the vault envelope.
//!
//! Design (see docs/security-architecture.md):
//!   passphrase --Argon2id(salt, params)--> KEK (32 bytes, ephemeral)
//!   KEK --XChaCha20-Poly1305(AAD = public header)--> wrapped random DEK
//!   DEK --SQLCipher raw key--> live encrypted ledger
//!
//! No secret type here implements Debug, Clone, Serialize, or Display.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroize;

use crate::error::{AppError, AppResult};

pub const DEK_LEN: usize = 32;
pub const KEK_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const AEAD_TAG_LEN: usize = 16;
pub const WRAPPED_DEK_LEN: usize = DEK_LEN + AEAD_TAG_LEN;

pub const MIN_PASSPHRASE_CHARS: usize = 16;
pub const MAX_PASSPHRASE_CHARS: usize = 1024;

/// A passphrase held only in Rust memory. Deliberately not Debug/Clone/Serialize.
pub struct SecretString {
    inner: zeroize::Zeroizing<String>,
}

impl SecretString {
    pub fn new(value: String) -> Self {
        Self { inner: zeroize::Zeroizing::new(value) }
    }

    /// Exact bytes as typed: no trimming, case folding, or Unicode normalization.
    pub fn expose(&self) -> &str {
        self.inner.as_str()
    }

    pub fn char_len(&self) -> usize {
        self.inner.chars().count()
    }
}

/// A 256-bit key whose page is best-effort `mlock`ed and zeroized on drop.
pub struct LockedKey {
    bytes: Box<[u8; KEK_LEN]>,
    locked: bool,
}

impl LockedKey {
    pub fn new(bytes: [u8; KEK_LEN]) -> Self {
        let boxed = Box::new(bytes);
        // Best effort: mlock can fail under memory pressure or sandbox limits.
        let locked = unsafe { libc::mlock(boxed.as_ptr() as *const libc::c_void, KEK_LEN) == 0 };
        // `bytes` was moved into the box; no plaintext stack copy outlives this.
        Self { bytes: boxed, locked }
    }

    pub fn generate() -> AppResult<Self> {
        let mut bytes = [0u8; KEK_LEN];
        getrandom::fill(&mut bytes).map_err(|_| AppError::Internal)?;
        Ok(Self::new(bytes))
    }

    pub fn from_slice(slice: &[u8]) -> AppResult<Self> {
        if slice.len() != KEK_LEN {
            return Err(AppError::Internal);
        }
        let mut bytes = [0u8; KEK_LEN];
        bytes.copy_from_slice(slice);
        Ok(Self::new(bytes))
    }

    pub fn expose(&self) -> &[u8; KEK_LEN] {
        &self.bytes
    }

    pub fn hex(&self) -> String {
        hex::encode(self.expose())
    }

    pub fn is_mlocked(&self) -> bool {
        self.locked
    }
}

impl Drop for LockedKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
        if self.locked {
            unsafe {
                libc::munlock(self.bytes.as_ptr() as *const libc::c_void, KEK_LEN);
            }
        }
    }
}

/// Argon2id parameters stored in the authenticated vault header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        // Spec default: 64 MiB, 3 iterations, 1 lane. Calibrated at creation
        // within the hard bounds below.
        Self { m_cost_kib: 65_536, t_cost: 3, p_cost: 1 }
    }
}

impl KdfParams {
    pub const MIN_M_COST_KIB: u32 = 8 * 1024;
    pub const MAX_M_COST_KIB: u32 = 256 * 1024;
    pub const MIN_T_COST: u32 = 1;
    pub const MAX_T_COST: u32 = 10;
    pub const MIN_P_COST: u32 = 1;
    pub const MAX_P_COST: u32 = 4;

    /// Validate before any KDF work so a tampered header cannot cause a DoS.
    pub fn validate(&self) -> AppResult<()> {
        if self.m_cost_kib < Self::MIN_M_COST_KIB
            || self.m_cost_kib > Self::MAX_M_COST_KIB
            || self.t_cost < Self::MIN_T_COST
            || self.t_cost > Self::MAX_T_COST
            || self.p_cost < Self::MIN_P_COST
            || self.p_cost > Self::MAX_P_COST
            // Argon2 requires at least 8 KiB per lane.
            || self.m_cost_kib < 8 * self.p_cost
        {
            return Err(AppError::KdfOutOfBounds);
        }
        Ok(())
    }

    pub fn human_summary(&self) -> String {
        format!(
            "Argon2id, {} MiB memory, {} iterations, {} lane{}",
            self.m_cost_kib / 1024,
            self.t_cost,
            self.p_cost,
            if self.p_cost == 1 { "" } else { "s" }
        )
    }
}

pub fn derive_kek(
    passphrase: &SecretString,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
) -> AppResult<LockedKey> {
    params.validate()?;
    let argon_params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(KEK_LEN))
        .map_err(|_| AppError::KdfOutOfBounds)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = [0u8; KEK_LEN];
    argon
        .hash_password_into(passphrase.expose().as_bytes(), salt, &mut out)
        .map_err(|_| AppError::Internal)?;
    Ok(LockedKey::new(out))
}

/// Calibrate within hard bounds at vault creation. Never trusts stored,
/// unvalidated parameters on open.
pub fn calibrate_kdf() -> KdfParams {
    use std::time::Instant;
    let salt = [7u8; SALT_LEN];
    let probe = SecretString::new("calibration-probe-passphrase".to_string());
    let mut params = KdfParams::default();
    loop {
        let start = Instant::now();
        if derive_kek(&probe, &salt, &params).is_err() {
            return KdfParams::default();
        }
        let elapsed = start.elapsed();
        if elapsed >= std::time::Duration::from_millis(350)
            || params.t_cost >= KdfParams::MAX_T_COST
        {
            return params;
        }
        params.t_cost += 1;
    }
}

/// Conservative passphrase policy: at least 16 characters OR ~60 estimated bits.
/// Spaces and long passphrases are allowed; no composition rules.
pub fn validate_new_passphrase(passphrase: &SecretString) -> AppResult<()> {
    let len = passphrase.char_len();
    if len == 0 || len > MAX_PASSPHRASE_CHARS {
        return Err(AppError::InvalidInput("passphrase length"));
    }
    if len >= MIN_PASSPHRASE_CHARS {
        return Ok(());
    }
    if estimate_entropy_bits(passphrase.expose()) >= 60 {
        return Ok(());
    }
    Err(AppError::InvalidInput("passphrase is too weak"))
}

/// Deliberately conservative: class-pool estimate with a repeated-character
/// penalty. It can only weaken the 16-character rule, never override it.
pub fn estimate_entropy_bits(value: &str) -> u32 {
    if value.is_empty() {
        return 0;
    }
    let has_lower = value.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = value.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = value.chars().any(|c| c.is_ascii_digit());
    let has_symbol = value.chars().any(|c| !c.is_alphanumeric() && !c.is_whitespace());
    let has_space = value.chars().any(char::is_whitespace);
    let mut pool = 0f64;
    if has_lower { pool += 26.0; }
    if has_upper { pool += 26.0; }
    if has_digit { pool += 10.0; }
    if has_symbol { pool += 33.0; }
    if has_space { pool += 1.0; }
    let pool = pool.max(2.0);
    let len = value.chars().count() as f64;
    let unique = value.chars().collect::<std::collections::HashSet<_>>().len() as f64;
    let repetition_penalty = (unique / len).clamp(0.25, 1.0);
    (len * pool.log2() * repetition_penalty).floor() as u32
}

pub fn seal(
    key: &LockedKey,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> AppResult<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose()).map_err(|_| AppError::Internal)?;
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .map_err(|_| AppError::Internal)
}

pub fn open_sealed(
    key: &LockedKey,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> AppResult<zeroize::Zeroizing<Vec<u8>>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose()).map_err(|_| AppError::Internal)?;
    let plaintext = cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map_err(|_| AppError::WrongPassphrase)?;
    Ok(zeroize::Zeroizing::new(plaintext))
}

pub fn random_array<const N: usize>() -> AppResult<[u8; N]> {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).map_err(|_| AppError::Internal)?;
    Ok(out)
}

pub fn random_vault_id() -> AppResult<[u8; 16]> {
    random_array::<16>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(s: &str) -> SecretString {
        SecretString::new(s.to_string())
    }

    #[test]
    fn kdf_bounds_reject_absurd_params() {
        let salt = [1u8; SALT_LEN];
        let p = KdfParams { m_cost_kib: 4 * 1024 * 1024, t_cost: 3, p_cost: 1 };
        assert!(matches!(derive_kek(&pass("x"), &salt, &p), Err(AppError::KdfOutOfBounds)));
        let p = KdfParams { m_cost_kib: 65_536, t_cost: 1000, p_cost: 1 };
        assert!(matches!(derive_kek(&pass("x"), &salt, &p), Err(AppError::KdfOutOfBounds)));
        let p = KdfParams { m_cost_kib: 65_536, t_cost: 3, p_cost: 0 };
        assert!(matches!(derive_kek(&pass("x"), &salt, &p), Err(AppError::KdfOutOfBounds)));
    }

    #[test]
    fn passphrase_policy_allows_long_with_spaces_and_rejects_weak() {
        assert!(validate_new_passphrase(&pass("correct horse battery staple")).is_ok());
        assert!(validate_new_passphrase(&pass("short")).is_err());
        assert!(validate_new_passphrase(&pass("")).is_err());
        assert!(validate_new_passphrase(&pass(&"a".repeat(1025))).is_err());
        // Spaces and case preserved exactly.
        let p = pass("  Mixed CASE passphrase with spaces  ");
        assert_eq!(p.expose(), "  Mixed CASE passphrase with spaces  ");
    }

    #[test]
    fn seal_open_roundtrip_and_tamper() {
        let key = LockedKey::generate().unwrap();
        let nonce = random_array::<NONCE_LEN>().unwrap();
        let aad = b"header-aad";
        let ct = seal(&key, &nonce, aad, b"dek-material").unwrap();
        let pt = open_sealed(&key, &nonce, aad, &ct).unwrap();
        assert_eq!(&pt[..], b"dek-material");
        let mut bad = ct.clone();
        bad[0] ^= 1;
        assert!(matches!(open_sealed(&key, &nonce, aad, &bad), Err(AppError::WrongPassphrase)));
        assert!(matches!(open_sealed(&key, &nonce, b"other-aad", &ct), Err(AppError::WrongPassphrase)));
    }

    #[test]
    fn kdf_is_deterministic_and_salt_sensitive() {
        let p = KdfParams { m_cost_kib: 8 * 1024, t_cost: 1, p_cost: 1 };
        let a = derive_kek(&pass("passphrase-with-enough-length"), &[1u8; 16], &p).unwrap();
        let b = derive_kek(&pass("passphrase-with-enough-length"), &[1u8; 16], &p).unwrap();
        let c = derive_kek(&pass("passphrase-with-enough-length"), &[2u8; 16], &p).unwrap();
        assert_eq!(a.expose(), b.expose());
        assert_ne!(a.expose(), c.expose());
    }
}
