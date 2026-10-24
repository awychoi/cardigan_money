//! Structured application errors.
//!
//! Security rule: errors that cross the IPC boundary contain a stable code and a
//! sanitized message only. They never contain SQL, filesystem paths, tokens,
//! account numbers, passphrases, or provider payloads.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("vault is missing")]
    VaultMissing,
    #[error("vault already exists")]
    VaultExists,
    #[error("wrong passphrase")]
    WrongPassphrase,
    #[error("vault header is invalid: {0}")]
    HeaderInvalid(&'static str),
    #[error("vault header is missing")]
    HeaderMissing,
    #[error("KDF parameters are outside the allowed bounds")]
    KdfOutOfBounds,
    #[error("SQLCipher is not active: {0}")]
    CipherUnavailable(String),
    #[error("database is not encrypted")]
    PlaintextDatabase,
    #[error("encryption integrity check failed")]
    IntegrityFailure,
    #[error("database error")]
    Database,
    #[error("I/O error")]
    Io,
    #[error("vault is locked")]
    Locked,
    #[error("operation cancelled")]
    Cancelled,
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    #[error("not found")]
    NotFound,
    #[error("permission denied: {0}")]
    PermissionDenied(&'static str),
    #[error("unsupported operation")]
    Unsupported,
    #[error("blocked: {0}")]
    Blocked(&'static str),
    #[error("native secure input failed")]
    NativeInput,
    #[error("credential store failed")]
    Keychain,
    #[error("provider is not configured")]
    ProviderNotConfigured,
    #[error("provider error: {0}")]
    Provider(&'static str),
    #[error("too many attempts")]
    RateLimited,
    #[error("schema version is newer than this build")]
    SchemaTooNew,
    #[error("internal error")]
    Internal,
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::VaultMissing => "vault_missing",
            AppError::VaultExists => "vault_exists",
            AppError::WrongPassphrase => "wrong_passphrase",
            AppError::HeaderInvalid(_) => "vault_header_invalid",
            AppError::HeaderMissing => "vault_header_missing",
            AppError::KdfOutOfBounds => "kdf_out_of_bounds",
            AppError::CipherUnavailable(_) => "cipher_unavailable",
            AppError::PlaintextDatabase => "plaintext_database",
            AppError::IntegrityFailure => "integrity_failure",
            AppError::Database => "database_error",
            AppError::Io => "io_error",
            AppError::Locked => "vault_locked",
            AppError::Cancelled => "cancelled",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::NotFound => "not_found",
            AppError::PermissionDenied(_) => "permission_denied",
            AppError::Unsupported => "unsupported",
            AppError::Blocked(_) => "blocked",
            AppError::NativeInput => "native_input_failed",
            AppError::Keychain => "keychain_error",
            AppError::ProviderNotConfigured => "provider_not_configured",
            AppError::Provider(message) => {
                let _ = message;
                "provider_error"
            }
            AppError::RateLimited => "rate_limited",
            AppError::SchemaTooNew => "schema_too_new",
            AppError::Internal => "internal_error",
        }
    }

    /// A short message safe to show in the renderer. Static text only.
    pub fn safe_message(&self) -> String {
        match self {
            AppError::HeaderInvalid(_) => "The vault header is damaged or has been tampered with.".into(),
            AppError::CipherUnavailable(_) => "The encrypted database engine is not available; refusing to continue.".into(),
            AppError::PlaintextDatabase => "This file is not an encrypted ledger. Refusing to open it.".into(),
            AppError::IntegrityFailure => "The encrypted ledger failed its integrity check.".into(),
            AppError::Database => "The ledger could not be read or written.".into(),
            AppError::Io => "A local file operation failed.".into(),
            AppError::PermissionDenied(_) => "A local file or folder has unsafe permissions.".into(),
            AppError::InvalidInput(_) => "One or more values were invalid.".into(),
            AppError::Blocked(m) => format!("This action is unavailable: {m}"),
            AppError::ProviderNotConfigured => "No provider credentials are configured. Add Sandbox credentials in Settings first.".into(),
            AppError::Provider(message) => message.to_string(),
            AppError::SchemaTooNew => "This ledger was created by a newer app version. Update the app before opening it.".into(),
            other => other.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        AppError::Io
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        AppError::Database
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

impl From<&AppError> for ErrorPayload {
    fn from(e: &AppError) -> Self {
        Self { code: e.code().to_string(), message: e.safe_message() }
    }
}

impl From<AppError> for ErrorPayload {
    fn from(e: AppError) -> Self {
        ErrorPayload::from(&e)
    }
}

pub type AppResult<T> = Result<T, AppError>;
