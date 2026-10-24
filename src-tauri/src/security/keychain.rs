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
pub const ACCOUNT_CLIENT_ID: &str = "plaid-client-id-sandbox";
pub const ACCOUNT_SECRET: &str = "plaid-secret-sandbox";

pub trait CredentialStore: Send + Sync {
    fn set(&self, account: &str, secret: &SecretString) -> AppResult<()>;
    fn get(&self, account: &str) -> AppResult<Option<SecretString>>;
    fn delete(&self, account: &str) -> AppResult<()>;
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
}

#[cfg(target_os = "macos")]
fn keychain_set(service: &str, account: &str, value: &[u8]) -> AppResult<()> {
    use core_foundation::base::{TCFType, CFType};
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
    use security_framework_sys::keychain_item::{SecItemAdd, SecItemDelete};

    // Remove any prior item first so the accessibility class cannot be
    // silently inherited from an older item.
    let _ = keychain_delete(service, account);
    let accessible_key = CFString::new("accessible");
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
fn keychain_get(service: &str, account: &str) -> AppResult<Option<SecretString>> {
    use core_foundation::base::{TCFType, CFType, CFTypeRef};
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
    use core_foundation::base::{TCFType, CFType};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes() {
        let store = MemoryStore::default();
        let secret = SecretString::new("sandbox-secret-value".into());
        store.set(ACCOUNT_SECRET, &secret).unwrap();
        assert_eq!(store.get(ACCOUNT_SECRET).unwrap().unwrap().expose(), "sandbox-secret-value");
        store.delete(ACCOUNT_SECRET).unwrap();
        assert!(store.get(ACCOUNT_SECRET).unwrap().is_none());
    }
}
