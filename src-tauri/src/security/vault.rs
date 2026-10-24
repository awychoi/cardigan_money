//! Authenticated external vault header and rotation journal.
//!
//! The header lives outside the encrypted database. It holds the KDF
//! parameters, salt, wrapped DEK, vault identity and generation. All public
//! fields are bound as AEAD associated data, so any modification fails the
//! unwrap. The wrapped DEK is useless without the passphrase.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::security::crypto::{
    self, derive_kek, KdfParams, LockedKey, SecretString, NONCE_LEN, SALT_LEN, WRAPPED_DEK_LEN,
};
use crate::security::files::{
    atomic_write_private, ensure_private_dir, file_exists_no_follow, read_private_file,
    remove_file_if_exists, MAX_HEADER_BYTES,
};

const MAGIC: &[u8; 8] = b"CMVAULT1";
const J_MAGIC: &[u8; 8] = b"CMROTAT1";
pub const FORMAT_VERSION: u16 = 1;
pub const CIPHER_ID_SQLCIPHER4: u8 = 1;
pub const KDF_ID_ARGON2ID: u8 = 1;

const OFF_MAGIC: usize = 0;
const OFF_VERSION: usize = 8;
const OFF_CIPHER: usize = 10;
const OFF_KDF: usize = 11;
const OFF_M: usize = 12;
const OFF_T: usize = 16;
const OFF_P: usize = 20;
const OFF_SALT: usize = 24;
const OFF_VAULT: usize = 40;
const OFF_GEN: usize = 56;
const OFF_SCHEMA: usize = 60;
const OFF_CREATED: usize = 64;
const OFF_ROTATED: usize = 72;
const OFF_NONCE: usize = 80;
const OFF_WRAPPED: usize = 104;
pub const AAD_LEN: usize = OFF_WRAPPED;
pub const HEADER_LEN: usize = OFF_WRAPPED + WRAPPED_DEK_LEN;

#[derive(Clone)]
pub struct VaultHeader {
    pub vault_id: [u8; 16],
    pub params: KdfParams,
    pub salt: [u8; SALT_LEN],
    pub generation: u32,
    pub schema_version: u32,
    pub created_at: i64,
    pub rotated_at: i64,
    pub wrap_nonce: [u8; NONCE_LEN],
    pub wrapped_dek: [u8; WRAPPED_DEK_LEN],
}

impl std::fmt::Debug for VaultHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultHeader")
            .field("vault_id", &hex::encode(self.vault_id))
            .field("params", &self.params)
            .field("generation", &self.generation)
            .field("schema_version", &self.schema_version)
            .finish_non_exhaustive()
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl VaultHeader {
    /// Create a brand new header and the random DEK it wraps.
    pub fn create(
        passphrase: &SecretString,
        params: KdfParams,
        vault_id: [u8; 16],
        schema_version: u32,
    ) -> AppResult<(Self, LockedKey)> {
        params.validate()?;
        let dek = LockedKey::generate()?;
        let salt = crypto::random_array::<SALT_LEN>()?;
        let nonce = crypto::random_array::<NONCE_LEN>()?;
        let kek = derive_kek(passphrase, &salt, &params)?;
        let mut header = Self {
            vault_id,
            params,
            salt,
            generation: 1,
            schema_version,
            created_at: now_unix(),
            rotated_at: 0,
            wrap_nonce: nonce,
            wrapped_dek: [0u8; WRAPPED_DEK_LEN],
        };
        let encoded = header.public_bytes();
        let wrapped = crypto::seal(&kek, &nonce, &encoded[..AAD_LEN], dek.expose())?;
        if wrapped.len() != WRAPPED_DEK_LEN {
            return Err(AppError::Internal);
        }
        header.wrapped_dek.copy_from_slice(&wrapped);
        Ok((header, dek))
    }

    /// Rotate to a new DEK, salt, nonce and generation under a new passphrase.
    pub fn rotate(
        &self,
        new_passphrase: &SecretString,
        params: KdfParams,
    ) -> AppResult<(Self, LockedKey)> {
        params.validate()?;
        let new_dek = LockedKey::generate()?;
        let salt = crypto::random_array::<SALT_LEN>()?;
        let nonce = crypto::random_array::<NONCE_LEN>()?;
        let kek = derive_kek(new_passphrase, &salt, &params)?;
        let mut header = Self {
            vault_id: self.vault_id,
            params,
            salt,
            generation: self.generation.saturating_add(1),
            schema_version: self.schema_version,
            created_at: self.created_at,
            rotated_at: now_unix(),
            wrap_nonce: nonce,
            wrapped_dek: [0u8; WRAPPED_DEK_LEN],
        };
        let encoded = header.public_bytes();
        let wrapped = crypto::seal(&kek, &nonce, &encoded[..AAD_LEN], new_dek.expose())?;
        header.wrapped_dek.copy_from_slice(&wrapped);
        Ok((header, new_dek))
    }

    fn public_bytes(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[OFF_MAGIC..OFF_MAGIC + 8].copy_from_slice(MAGIC);
        out[OFF_VERSION..OFF_VERSION + 2].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        out[OFF_CIPHER] = CIPHER_ID_SQLCIPHER4;
        out[OFF_KDF] = KDF_ID_ARGON2ID;
        out[OFF_M..OFF_M + 4].copy_from_slice(&self.params.m_cost_kib.to_le_bytes());
        out[OFF_T..OFF_T + 4].copy_from_slice(&self.params.t_cost.to_le_bytes());
        out[OFF_P..OFF_P + 4].copy_from_slice(&self.params.p_cost.to_le_bytes());
        out[OFF_SALT..OFF_SALT + SALT_LEN].copy_from_slice(&self.salt);
        out[OFF_VAULT..OFF_VAULT + 16].copy_from_slice(&self.vault_id);
        out[OFF_GEN..OFF_GEN + 4].copy_from_slice(&self.generation.to_le_bytes());
        out[OFF_SCHEMA..OFF_SCHEMA + 4].copy_from_slice(&self.schema_version.to_le_bytes());
        out[OFF_CREATED..OFF_CREATED + 8].copy_from_slice(&self.created_at.to_le_bytes());
        out[OFF_ROTATED..OFF_ROTATED + 8].copy_from_slice(&self.rotated_at.to_le_bytes());
        out[OFF_NONCE..OFF_NONCE + NONCE_LEN].copy_from_slice(&self.wrap_nonce);
        out[OFF_WRAPPED..OFF_WRAPPED + WRAPPED_DEK_LEN].copy_from_slice(&self.wrapped_dek);
        out
    }

    pub fn encode(&self) -> [u8; HEADER_LEN] {
        self.public_bytes()
    }

    pub fn decode(bytes: &[u8]) -> AppResult<Self> {
        if bytes.len() != HEADER_LEN {
            return Err(AppError::HeaderInvalid("length"));
        }
        if &bytes[OFF_MAGIC..OFF_MAGIC + 8] != MAGIC {
            return Err(AppError::HeaderInvalid("magic"));
        }
        let version = u16::from_le_bytes(bytes[OFF_VERSION..OFF_VERSION + 2].try_into().unwrap());
        if version != FORMAT_VERSION {
            return Err(AppError::HeaderInvalid("format version"));
        }
        if bytes[OFF_CIPHER] != CIPHER_ID_SQLCIPHER4 {
            return Err(AppError::HeaderInvalid("cipher id"));
        }
        if bytes[OFF_KDF] != KDF_ID_ARGON2ID {
            return Err(AppError::HeaderInvalid("kdf id"));
        }
        let params = KdfParams {
            m_cost_kib: u32::from_le_bytes(bytes[OFF_M..OFF_M + 4].try_into().unwrap()),
            t_cost: u32::from_le_bytes(bytes[OFF_T..OFF_T + 4].try_into().unwrap()),
            p_cost: u32::from_le_bytes(bytes[OFF_P..OFF_P + 4].try_into().unwrap()),
        };
        // Bound before any KDF work.
        params.validate()?;
        let generation = u32::from_le_bytes(bytes[OFF_GEN..OFF_GEN + 4].try_into().unwrap());
        let schema_version =
            u32::from_le_bytes(bytes[OFF_SCHEMA..OFF_SCHEMA + 4].try_into().unwrap());
        if generation == 0 || schema_version == 0 {
            return Err(AppError::HeaderInvalid("generation"));
        }
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&bytes[OFF_SALT..OFF_SALT + SALT_LEN]);
        let mut vault_id = [0u8; 16];
        vault_id.copy_from_slice(&bytes[OFF_VAULT..OFF_VAULT + 16]);
        let mut wrap_nonce = [0u8; NONCE_LEN];
        wrap_nonce.copy_from_slice(&bytes[OFF_NONCE..OFF_NONCE + NONCE_LEN]);
        let mut wrapped_dek = [0u8; WRAPPED_DEK_LEN];
        wrapped_dek.copy_from_slice(&bytes[OFF_WRAPPED..OFF_WRAPPED + WRAPPED_DEK_LEN]);
        Ok(Self {
            vault_id,
            params,
            salt,
            generation,
            schema_version,
            created_at: i64::from_le_bytes(bytes[OFF_CREATED..OFF_CREATED + 8].try_into().unwrap()),
            rotated_at: i64::from_le_bytes(bytes[OFF_ROTATED..OFF_ROTATED + 8].try_into().unwrap()),
            wrap_nonce,
            wrapped_dek,
        })
    }

    /// Derive the KEK and unwrap the DEK. Wrong passphrase and any header
    /// tampering both fail here (indistinguishable, fail closed).
    pub fn unwrap_dek(&self, passphrase: &SecretString) -> AppResult<LockedKey> {
        self.params.validate()?;
        let kek = derive_kek(passphrase, &self.salt, &self.params)?;
        let aad = self.public_bytes();
        let plaintext =
            crypto::open_sealed(&kek, &self.wrap_nonce, &aad[..AAD_LEN], &self.wrapped_dek)?;
        if plaintext.len() != crypto::DEK_LEN {
            return Err(AppError::HeaderInvalid("dek length"));
        }
        LockedKey::from_slice(&plaintext)
    }

    pub fn vault_id_hex(&self) -> String {
        hex::encode(self.vault_id)
    }
}

/// Paths used by a real vault. Demo mode uses a distinct root and no header.
#[derive(Clone, Debug)]
pub struct VaultPaths {
    pub root: PathBuf,
    pub header: PathBuf,
    pub header_mirror: PathBuf,
    pub journal: PathBuf,
    pub database: PathBuf,
}

impl VaultPaths {
    pub fn new(app_data: &Path) -> Self {
        let root = app_data.join("vault");
        Self {
            header: root.join("vault.header"),
            header_mirror: root.join("vault.header.bak"),
            journal: root.join("rotation.journal"),
            database: root.join("ledger.cmdb"),
            root,
        }
    }

    pub fn ensure(&self) -> AppResult<()> {
        ensure_private_dir(&self.root)
    }

    pub fn exists(&self) -> AppResult<bool> {
        file_exists_no_follow(&self.header)
    }
}

pub enum LoadedHeader {
    Primary(VaultHeader),
    Mirror(VaultHeader),
}

impl LoadedHeader {
    pub fn header(&self) -> &VaultHeader {
        match self {
            LoadedHeader::Primary(h) | LoadedHeader::Mirror(h) => h,
        }
    }

    pub fn from_mirror(&self) -> bool {
        matches!(self, LoadedHeader::Mirror(_))
    }
}

/// Load the newest valid header; fall back to the mirrored copy.
pub fn load_header(paths: &VaultPaths) -> AppResult<LoadedHeader> {
    let primary = read_private_file(&paths.header, MAX_HEADER_BYTES)
        .ok()
        .and_then(|b| VaultHeader::decode(&b).ok());
    let mirror = read_private_file(&paths.header_mirror, MAX_HEADER_BYTES)
        .ok()
        .and_then(|b| VaultHeader::decode(&b).ok());
    match (primary, mirror) {
        (Some(p), Some(m)) => {
            if m.generation > p.generation {
                Ok(LoadedHeader::Mirror(m))
            } else {
                Ok(LoadedHeader::Primary(p))
            }
        }
        (Some(p), None) => Ok(LoadedHeader::Primary(p)),
        (None, Some(m)) => Ok(LoadedHeader::Mirror(m)),
        (None, None) => {
            if !file_exists_no_follow(&paths.header)? {
                Err(AppError::VaultMissing)
            } else {
                Err(AppError::HeaderInvalid("no valid header"))
            }
        }
    }
}

/// Write both copies atomically, primary first then mirror.
pub fn write_header(paths: &VaultPaths, header: &VaultHeader) -> AppResult<()> {
    let bytes = header.encode();
    atomic_write_private(&paths.header, &bytes)?;
    atomic_write_private(&paths.header_mirror, &bytes)?;
    Ok(())
}

pub fn remove_header(paths: &VaultPaths) -> AppResult<()> {
    remove_file_if_exists(&paths.header)?;
    remove_file_if_exists(&paths.header_mirror)?;
    remove_file_if_exists(&paths.journal)?;
    Ok(())
}

pub struct RotationJournal {
    pub previous_generation: u32,
    pub new_header: VaultHeader,
}

impl RotationJournal {
    const LEN: usize = 8 + 4 + HEADER_LEN;

    pub fn new(previous_generation: u32, new_header: VaultHeader) -> Self {
        Self {
            previous_generation,
            new_header,
        }
    }

    pub fn encode(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        out[0..8].copy_from_slice(J_MAGIC);
        out[8..12].copy_from_slice(&self.previous_generation.to_le_bytes());
        out[12..].copy_from_slice(&self.new_header.encode());
        out
    }

    pub fn decode(bytes: &[u8]) -> AppResult<Self> {
        if bytes.len() != Self::LEN || &bytes[0..8] != J_MAGIC {
            return Err(AppError::HeaderInvalid("rotation journal"));
        }
        let previous_generation = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let new_header = VaultHeader::decode(&bytes[12..])?;
        Ok(Self {
            previous_generation,
            new_header,
        })
    }
}

pub fn write_journal(paths: &VaultPaths, journal: &RotationJournal) -> AppResult<()> {
    atomic_write_private(&paths.journal, &journal.encode())
}

pub fn load_journal(paths: &VaultPaths) -> AppResult<Option<RotationJournal>> {
    if !file_exists_no_follow(&paths.journal)? {
        return Ok(None);
    }
    let bytes = read_private_file(&paths.journal, MAX_HEADER_BYTES)?;
    RotationJournal::decode(&bytes).map(Some)
}

pub fn remove_journal(paths: &VaultPaths) -> AppResult<()> {
    remove_file_if_exists(&paths.journal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> SecretString {
        SecretString::new(s.to_string())
    }

    fn fast_params() -> KdfParams {
        KdfParams {
            m_cost_kib: 8 * 1024,
            t_cost: 1,
            p_cost: 1,
        }
    }

    #[test]
    fn header_roundtrip_wraps_and_unwraps() {
        let (header, dek) = VaultHeader::create(
            &p("correct horse battery staple"),
            fast_params(),
            [9u8; 16],
            1,
        )
        .unwrap();
        let bytes = header.encode();
        let decoded = VaultHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.vault_id, [9u8; 16]);
        assert_eq!(decoded.generation, 1);
        let unwrapped = decoded
            .unwrap_dek(&p("correct horse battery staple"))
            .unwrap();
        assert_eq!(unwrapped.expose(), dek.expose());
        assert_eq!(bytes.len(), HEADER_LEN);
        assert!(matches!(
            decoded.unwrap_dek(&p("wrong passphrase but long enough")),
            Err(AppError::WrongPassphrase)
        ));
    }

    #[test]
    fn every_header_byte_is_authenticated_or_validated() {
        let (header, _) = VaultHeader::create(
            &p("correct horse battery staple"),
            fast_params(),
            [3u8; 16],
            1,
        )
        .unwrap();
        let base = header.encode();
        // Flipping any byte in the public AAD region must fail decode or unwrap.
        for i in 0..AAD_LEN {
            let mut tampered = base;
            tampered[i] ^= 0x01;
            let result = VaultHeader::decode(&tampered)
                .and_then(|h| h.unwrap_dek(&p("correct horse battery staple")));
            assert!(result.is_err(), "byte {i} tamper was not detected");
        }
        // Wrapped DEK region failure is detected at unwrap.
        for i in AAD_LEN..HEADER_LEN {
            let mut tampered = base;
            tampered[i] ^= 0x01;
            let result = VaultHeader::decode(&tampered)
                .and_then(|h| h.unwrap_dek(&p("correct horse battery staple")));
            assert!(result.is_err(), "wrapped byte {i} tamper was not detected");
        }
        // Truncation.
        assert!(VaultHeader::decode(&base[..HEADER_LEN - 1]).is_err());
        assert!(VaultHeader::decode(&[]).is_err());
    }

    #[test]
    fn absurd_kdf_parameters_are_rejected_before_derivation() {
        let (header, _) = VaultHeader::create(
            &p("correct horse battery staple"),
            fast_params(),
            [3u8; 16],
            1,
        )
        .unwrap();
        let mut bytes = header.encode();
        // 4 TiB memory parameter.
        bytes[OFF_M..OFF_M + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let start = std::time::Instant::now();
        assert!(matches!(
            VaultHeader::decode(&bytes),
            Err(AppError::KdfOutOfBounds)
        ));
        assert!(start.elapsed() < std::time::Duration::from_millis(50));
        bytes[OFF_M..OFF_M + 4].copy_from_slice(&65_536u32.to_le_bytes());
        bytes[OFF_KDF] = 99;
        assert!(VaultHeader::decode(&bytes).is_err());
    }

    #[test]
    fn rotation_changes_dek_salt_generation() {
        let (header, old_dek) = VaultHeader::create(
            &p("correct horse battery staple"),
            fast_params(),
            [3u8; 16],
            1,
        )
        .unwrap();
        let (new_header, new_dek) = header
            .rotate(&p("new correct horse battery staple"), fast_params())
            .unwrap();
        assert_eq!(new_header.generation, 2);
        assert_ne!(old_dek.expose(), new_dek.expose());
        assert_ne!(header.salt, new_header.salt);
        assert!(new_header
            .unwrap_dek(&p("correct horse battery staple"))
            .is_err());
        assert_eq!(
            new_header
                .unwrap_dek(&p("new correct horse battery staple"))
                .unwrap()
                .expose(),
            new_dek.expose()
        );
        assert_eq!(
            header
                .unwrap_dek(&p("correct horse battery staple"))
                .unwrap()
                .expose(),
            old_dek.expose()
        );
    }

    #[test]
    fn header_files_atomic_and_mirrored() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(tmp.path());
        paths.ensure().unwrap();
        let (header, _) = VaultHeader::create(
            &p("correct horse battery staple"),
            fast_params(),
            [3u8; 16],
            1,
        )
        .unwrap();
        write_header(&paths, &header).unwrap();
        assert!(paths.header.exists());
        assert!(paths.header_mirror.exists());
        let loaded = load_header(&paths).unwrap();
        assert!(!loaded.from_mirror());
        // Corrupt primary: mirror still opens.
        std::fs::write(&paths.header, b"garbage").unwrap();
        let loaded = load_header(&paths).unwrap();
        assert!(loaded.from_mirror());
        // Remove both: vault missing.
        remove_header(&paths).unwrap();
        assert!(matches!(load_header(&paths), Err(AppError::VaultMissing)));
    }

    #[test]
    fn rotation_journal_roundtrip() {
        let (header, _) = VaultHeader::create(
            &p("correct horse battery staple"),
            fast_params(),
            [3u8; 16],
            1,
        )
        .unwrap();
        let (new_header, _) = header
            .rotate(&p("new correct horse battery staple"), fast_params())
            .unwrap();
        let journal = RotationJournal::new(header.generation, new_header);
        let decoded = RotationJournal::decode(&journal.encode()).unwrap();
        assert_eq!(decoded.previous_generation, 1);
        assert_eq!(decoded.new_header.generation, 2);
    }
}
