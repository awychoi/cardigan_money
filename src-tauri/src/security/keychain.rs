//! Minimal macOS Keychain wrapper for provider credentials.
//!
//! Generic-password items are created with
//! `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` and
//! `kSecAttrSynchronizable = false`. Values never enter the ledger, renderer,
//! logs, URLs, or exports. The Swift/AppKit prompt is not involved; Keychain
//! access is app-bound by the code signature/Keychain ACL.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{AppError, AppResult};
use crate::security::crypto::SecretString;

pub const KEYCHAIN_SERVICE: &str = "com.awychoi.cardiganmoney.plaid";

#[cfg(target_os = "macos")]
extern "C" {
    static kSecAttrAccessible: core_foundation_sys::string::CFStringRef;
    static kSecMatchLimitOne: core_foundation_sys::string::CFStringRef;
}
/// Plaid client credentials are stored per environment. Sandbox and
/// Production (including Trial) use different secrets and different hosts, so
/// they must never share a Keychain slot.
pub const ACCOUNT_CLIENT_ID_SANDBOX: &str = "plaid-client-id-sandbox";
pub const ACCOUNT_SECRET_SANDBOX: &str = "plaid-secret-sandbox";
pub const ACCOUNT_CLIENT_ID_PRODUCTION: &str = "plaid-client-id-production";
pub const ACCOUNT_SECRET_PRODUCTION: &str = "plaid-secret-production";

/// Every app-managed Plaid client credential. Used by crypto-erase, which must
/// not leave either environment's secret behind.
pub const PLAID_CLIENT_ACCOUNTS: &[&str] = &[
    ACCOUNT_CLIENT_ID_SANDBOX,
    ACCOUNT_SECRET_SANDBOX,
    ACCOUNT_CLIENT_ID_PRODUCTION,
    ACCOUNT_SECRET_PRODUCTION,
];

pub trait CredentialStore: Send + Sync {
    fn set(&self, account: &str, secret: &SecretString) -> AppResult<()>;
    fn get(&self, account: &str) -> AppResult<Option<SecretString>>;
    fn delete(&self, account: &str) -> AppResult<()>;
    /// Delete every app-managed credential whose account name starts with
    /// `prefix`. Used by the destructive no-passphrase restore path, which
    /// cannot read the old ledger to discover credential references.
    fn delete_prefix(&self, prefix: &str) -> AppResult<usize>;
}

#[derive(Debug, Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<String, String>>,
}

impl CredentialStore for MemoryStore {
    fn set(&self, account: &str, secret: &SecretString) -> AppResult<()> {
        let mut values = self.values.lock().map_err(|_| AppError::Internal)?;
        values.insert(account.to_string(), secret.expose().to_string());
        Ok(())
    }

    fn get(&self, account: &str) -> AppResult<Option<SecretString>> {
        let values = self.values.lock().map_err(|_| AppError::Internal)?;
        Ok(values.get(account).cloned().map(SecretString::new))
    }

    fn delete(&self, account: &str) -> AppResult<()> {
        let mut values = self.values.lock().map_err(|_| AppError::Internal)?;
        values.remove(account);
        Ok(())
    }

    fn delete_prefix(&self, prefix: &str) -> AppResult<usize> {
        let mut values = self.values.lock().map_err(|_| AppError::Internal)?;
        let before = values.len();
        values.retain(|account, _| !account.starts_with(prefix));
        Ok(before - values.len())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct KeychainStore;

impl CredentialStore for KeychainStore {
    fn set(&self, account: &str, secret: &SecretString) -> AppResult<()> {
        keychain_set(KEYCHAIN_SERVICE, account, secret.expose().as_bytes())
    }

    fn get(&self, account: &str) -> AppResult<Option<SecretString>> {
        keychain_get(KEYCHAIN_SERVICE, account)
    }

    fn delete(&self, account: &str) -> AppResult<()> {
        keychain_delete(KEYCHAIN_SERVICE, account)
    }

    fn delete_prefix(&self, prefix: &str) -> AppResult<usize> {
        keychain_delete_prefix(KEYCHAIN_SERVICE, prefix)
    }
}

#[cfg(target_os = "macos")]
fn keychain_set(service: &str, account: &str, value: &[u8]) -> AppResult<()> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use security_framework_sys::access_control::kSecAttrAccessibleWhenUnlockedThisDeviceOnly;
    use security_framework_sys::base::errSecSuccess;
    use security_framework_sys::item::{
        kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecClass,
        kSecClassGenericPassword, kSecValueData,
    };
    use security_framework_sys::keychain_item::SecItemAdd;

    // Remove any prior item first so the accessibility class cannot be
    // silently inherited from an older item.
    let _ = keychain_delete(service, account);
    let accessible_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccessible) };
    let accessible_value =
        unsafe { CFString::wrap_under_get_rule(kSecAttrAccessibleWhenUnlockedThisDeviceOnly) };
    let pairs: Vec<(CFString, CFType)> = vec![
        (
            unsafe { CFString::wrap_under_get_rule(kSecClass) },
            unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) }.into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrService) },
            CFString::new(service).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) },
            CFString::new(account).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecValueData) },
            CFData::from_buffer(value).into_CFType(),
        ),
        (accessible_key, accessible_value.into_CFType()),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrSynchronizable) },
            CFBoolean::from(false).into_CFType(),
        ),
    ];
    let dictionary = CFDictionary::<CFString, CFType>::from_CFType_pairs(&pairs);
    let status = unsafe { SecItemAdd(dictionary.as_concrete_TypeRef(), std::ptr::null_mut()) };
    if status == errSecSuccess {
        Ok(())
    } else {
        Err(AppError::Keychain)
    }
}

#[cfg(target_os = "macos")]
fn keychain_delete_prefix(service: &str, account_prefix: &str) -> AppResult<usize> {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::CFTypeRef;
    use core_foundation_sys::string::CFStringRef;
    use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};
    use security_framework_sys::item::kSecAttrAccount;

    let mut options = ItemSearchOptions::new();
    options
        .class(ItemClass::generic_password())
        .service(service)
        .load_attributes(true)
        .limit(Limit::All);
    let results = options.search().map_err(|_| AppError::Keychain)?;
    let key = unsafe { kSecAttrAccount as *const std::ffi::c_void };
    let mut deleted = 0usize;
    for result in results {
        let SearchResult::Dict(dict) = result else {
            continue;
        };
        let dict: CFDictionary = dict.into_untyped();
        let Some(value) = dict.find(key) else {
            continue;
        };
        let raw: CFTypeRef = *value as CFTypeRef;
        if raw.is_null() {
            continue;
        }
        let account = unsafe { CFString::wrap_under_get_rule(raw as CFStringRef) }.to_string();
        if account.starts_with(account_prefix) {
            keychain_delete(service, &account)?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

#[cfg(target_os = "macos")]
fn keychain_get(service: &str, account: &str) -> AppResult<Option<SecretString>> {
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::{CFData, CFDataRef};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{CFGetTypeID, CFRelease};
    use security_framework_sys::base::{errSecItemNotFound, errSecSuccess};
    use security_framework_sys::item::{
        kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecClass,
        kSecClassGenericPassword, kSecReturnData,
    };
    use security_framework_sys::keychain_item::SecItemCopyMatching;

    let pairs: Vec<(CFString, CFType)> = vec![
        (
            unsafe { CFString::wrap_under_get_rule(kSecClass) },
            unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) }.into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrService) },
            CFString::new(service).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) },
            CFString::new(account).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrSynchronizable) },
            CFBoolean::from(false).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecReturnData) },
            CFBoolean::from(true).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecMatchLimitOne) },
            unsafe { CFString::wrap_under_get_rule(kSecMatchLimitOne) }.into_CFType(),
        ),
    ];
    let dictionary = CFDictionary::<CFString, CFType>::from_CFType_pairs(&pairs);
    let mut result: CFTypeRef = std::ptr::null();
    let status = unsafe { SecItemCopyMatching(dictionary.as_concrete_TypeRef(), &mut result) };
    if status == errSecItemNotFound {
        return Ok(None);
    }
    if status != errSecSuccess || result.is_null() {
        return Err(AppError::Keychain);
    }
    if unsafe { CFGetTypeID(result) } != CFData::type_id() {
        unsafe { CFRelease(result) };
        return Err(AppError::Keychain);
    }
    let data = unsafe { CFData::wrap_under_create_rule(result as CFDataRef) };
    let bytes = data.bytes().to_vec();
    let value = String::from_utf8(bytes).map_err(|_| AppError::Keychain)?;
    Ok(Some(SecretString::new(value)))
}

#[cfg(target_os = "macos")]
fn keychain_delete(service: &str, account: &str) -> AppResult<()> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use security_framework_sys::base::{errSecItemNotFound, errSecSuccess};
    use security_framework_sys::item::{
        kSecAttrAccount, kSecAttrService, kSecAttrSynchronizable, kSecClass,
        kSecClassGenericPassword,
    };
    use security_framework_sys::keychain_item::SecItemDelete;

    let pairs: Vec<(CFString, CFType)> = vec![
        (
            unsafe { CFString::wrap_under_get_rule(kSecClass) },
            unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) }.into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrService) },
            CFString::new(service).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) },
            CFString::new(account).into_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrSynchronizable) },
            CFBoolean::from(false).into_CFType(),
        ),
    ];
    let dictionary = CFDictionary::<CFString, CFType>::from_CFType_pairs(&pairs);
    let status = unsafe { SecItemDelete(dictionary.as_concrete_TypeRef()) };
    if status == errSecSuccess || status == errSecItemNotFound {
        Ok(())
    } else {
        Err(AppError::Keychain)
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_set(_service: &str, _account: &str, _value: &[u8]) -> AppResult<()> {
    Err(AppError::Unsupported)
}

#[cfg(not(target_os = "macos"))]
fn keychain_get(_service: &str, _account: &str) -> AppResult<Option<SecretString>> {
    Err(AppError::Unsupported)
}

#[cfg(not(target_os = "macos"))]
fn keychain_delete(_service: &str, _account: &str) -> AppResult<()> {
    Err(AppError::Unsupported)
}

#[cfg(not(target_os = "macos"))]
fn keychain_delete_prefix(_service: &str, _account_prefix: &str) -> AppResult<usize> {
    Err(AppError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes() {
        let store = MemoryStore::default();
        let secret = SecretString::new("sandbox-secret-value".into());
        store.set(ACCOUNT_SECRET_SANDBOX, &secret).unwrap();
        assert_eq!(
            store.get(ACCOUNT_SECRET_SANDBOX).unwrap().unwrap().expose(),
            "sandbox-secret-value"
        );
        store.delete(ACCOUNT_SECRET_SANDBOX).unwrap();
        assert!(store.get(ACCOUNT_SECRET_SANDBOX).unwrap().is_none());
    }

    #[test]
    fn memory_store_deletes_only_matching_prefixes() {
        let store = MemoryStore::default();
        for account in [
            "plaid-access-token-one",
            "plaid-access-token-two",
            ACCOUNT_CLIENT_ID_SANDBOX,
            ACCOUNT_SECRET_SANDBOX,
            ACCOUNT_CLIENT_ID_PRODUCTION,
            ACCOUNT_SECRET_PRODUCTION,
        ] {
            store
                .set(account, &SecretString::new(format!("value-{account}")))
                .unwrap();
        }
        assert_eq!(store.delete_prefix("plaid-access-token-").unwrap(), 2);
        assert!(store.get("plaid-access-token-one").unwrap().is_none());
        assert!(store.get(ACCOUNT_SECRET_SANDBOX).unwrap().is_some());
        assert!(store.get(ACCOUNT_SECRET_PRODUCTION).unwrap().is_some());
    }
}
