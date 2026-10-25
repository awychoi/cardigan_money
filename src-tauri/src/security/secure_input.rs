//! Native macOS secure input.
//!
//! The ledger passphrase, new passphrases, backup/export passwords, and the
//! Plaid client secret are collected by an AppKit `NSSecureTextField` inside a
//! modal NSAlert. The renderer only ever learns success, failure, or
//! cancellation. There is deliberately no HTML password field fallback.

use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::security::crypto::SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurePurpose {
    CreateVault,
    Unlock,
    ChangeCurrent,
    ChangeNew,
    RestoreBackup,
    RestoreCurrentVault,
    RestoreChangeCurrentNew,
    BackupPassphrase,
    BackupUnlock,
    PlaidSetup,
}

#[derive(Debug, Clone, Copy)]
struct PromptSpec {
    title: &'static str,
    message: &'static str,
    labels: &'static [&'static str],
    action: &'static str,
    cancel: &'static str,
}

fn spec(purpose: SecurePurpose) -> PromptSpec {
    match purpose {
        SecurePurpose::CreateVault => PromptSpec {
            title: "Create your vault passphrase",
            message: "Choose a passphrase of at least 16 characters. Spaces and case are preserved exactly. There is NO recovery if you lose it, and no one can reset it. This sheet is native; the app window never sees the passphrase.",
            labels: &["Passphrase", "Confirm passphrase"],
            action: "Create vault",
            cancel: "Cancel",
        },
        SecurePurpose::Unlock => PromptSpec {
            title: "Unlock cardigan_money",
            message: "Enter your vault passphrase. It is never stored, logged, or sent anywhere.",
            labels: &["Passphrase"],
            action: "Unlock",
            cancel: "Cancel",
        },
        SecurePurpose::ChangeCurrent => PromptSpec {
            title: "Change vault passphrase",
            message: "Enter your current vault passphrase to continue.",
            labels: &["Current passphrase"],
            action: "Continue",
            cancel: "Cancel",
        },
        SecurePurpose::ChangeNew => PromptSpec {
            title: "Choose a new passphrase",
            message: "At least 16 characters. There is NO recovery if you lose it. Older backups will still require the old passphrase.",
            labels: &["New passphrase", "Confirm new passphrase"],
            action: "Change passphrase",
            cancel: "Cancel",
        },
        SecurePurpose::RestoreBackup => PromptSpec {
            title: "Restore an encrypted backup",
            message: "Enter the passphrase for this backup. Restoring replaces the current ledger only after validation succeeds.",
            labels: &["Backup passphrase"],
            action: "Restore",
            cancel: "Cancel",
        },
        SecurePurpose::RestoreCurrentVault => PromptSpec {
            title: "Restore into a locked vault",
            message: "Enter the current vault passphrase. It is used to make a pre-restore backup and to keep this vault's identity and passphrase. A wrong passphrase makes no changes.",
            labels: &["Current vault passphrase"],
            action: "Continue",
            cancel: "Cancel",
        },
        SecurePurpose::RestoreChangeCurrentNew => PromptSpec {
            title: "Change the vault passphrase after restore",
            message: "Enter the current vault passphrase, then choose a new passphrase of at least 16 characters. If you cancel, the restored vault keeps its current passphrase.",
            labels: &[
                "Current vault passphrase",
                "New passphrase",
                "Confirm new passphrase",
            ],
            action: "Change passphrase",
            cancel: "Cancel",
        },
        SecurePurpose::BackupPassphrase => PromptSpec {
            title: "Back up cardigan_money",
            message: "Leave this blank to use your vault passphrase. Enter a different passphrase to encrypt only this backup; a separate passphrase must be at least 16 characters. The app never stores it.",
            labels: &["Backup passphrase (blank = vault passphrase)"],
            action: "Continue",
            cancel: "Cancel",
        },
        SecurePurpose::BackupUnlock => PromptSpec {
            title: "Back up a locked vault",
            message: "Enter the current vault passphrase so the app can read the ledger for this backup. Optionally enter a different backup passphrase; leave it blank to use the vault passphrase. The app stays locked and shows no ledger data.",
            labels: &["Vault passphrase", "Backup passphrase (optional)"],
            action: "Continue",
            cancel: "Cancel",
        },
        SecurePurpose::PlaidSetup => PromptSpec {
            title: "Plaid Sandbox credentials",
            message: "Enter the Plaid client ID and Sandbox secret. They are stored only in the macOS Keychain (WhenUnlockedThisDeviceOnly, not synchronized), never in the ledger, logs, renderer, or exports. Use Sandbox credentials only for this phase.",
            labels: &["Client ID", "Sandbox secret"],
            action: "Save credentials",
            cancel: "Cancel",
        },
    }
}

/// Ask for one or more secrets. Returns `None` when the user cancels.
/// Any field may be empty; policy validation is the caller's job.
pub fn ask<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    purpose: SecurePurpose,
) -> AppResult<Option<Vec<SecretString>>> {
    show_native(app, spec(purpose))
}

/// Ask for a new passphrase twice and require an exact match.
pub fn ask_new_confirmed<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    purpose: SecurePurpose,
) -> AppResult<Option<SecretString>> {
    let Some(values) = ask(app, purpose)? else {
        return Ok(None);
    };
    if values.len() != 2 {
        return Err(AppError::NativeInput);
    }
    let matches: bool = values[0]
        .expose()
        .as_bytes()
        .ct_eq(values[1].expose().as_bytes())
        .into();
    if !matches {
        return Err(AppError::InvalidInput("passphrases do not match"));
    }
    let mut values = values;
    let confirmation = values.pop();
    let secret = values.pop().ok_or(AppError::NativeInput)?;
    drop(confirmation);
    Ok(Some(secret))
}

use subtle::ConstantTimeEq;

#[cfg(target_os = "macos")]
fn show_native<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    spec: PromptSpec,
) -> AppResult<Option<Vec<SecretString>>> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = unsafe { present_prompt(spec, None) };
        let _ = tx.send(result);
    })
    .map_err(|_| AppError::NativeInput)?;

    match rx.recv_timeout(Duration::from_secs(600)) {
        Ok(Ok(Some(values))) => Ok(Some(values.into_iter().map(SecretString::new).collect())),
        Ok(Ok(None)) => Ok(None),
        _ => Err(AppError::NativeInput),
    }
}

#[cfg(target_os = "macos")]
unsafe fn present_prompt(
    spec: PromptSpec,
    auto_cancel_after: Option<Duration>,
) -> Result<Option<Vec<String>>, ()> {
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSAlert, NSAlertFirstButtonReturn, NSControl, NSView, NSWindow, NSWindowSharingType,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

    let mtm = MainThreadMarker::new().ok_or(())?;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(spec.title));
    alert.setInformativeText(&NSString::from_str(spec.message));
    let _action = alert.addButtonWithTitle(&NSString::from_str(spec.action));
    let _cancel = alert.addButtonWithTitle(&NSString::from_str(spec.cancel));

    let row_height = 30.0;
    let width = 380.0;
    let height = row_height * spec.labels.len() as f64 + 12.0;
    let container = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)),
    );

    let mut fields = Vec::new();
    for (index, label) in spec.labels.iter().enumerate() {
        use objc2_app_kit::NSSecureTextField;
        let y = height - 12.0 - row_height * (index as f64 + 1.0) + 6.0;
        let field = NSSecureTextField::new(mtm);
        field.setFrame(NSRect::new(NSPoint::new(0.0, y), NSSize::new(width, 24.0)));
        field.setPlaceholderString(Some(&NSString::from_str(label)));
        container.addSubview(&field);
        fields.push(field);
    }

    alert.setAccessoryView(Some(&container));
    let window: objc2::rc::Retained<NSWindow> = alert.window();
    // Best-effort content protection for the sheet, same as the main window.
    window.setSharingType(NSWindowSharingType::None);
    if let Some(first) = fields.first() {
        window.setInitialFirstResponder(Some(first));
    }

    if let Some(delay) = auto_cancel_after {
        use block2::RcBlock;
        use objc2::MainThreadMarker as TimerMarker;
        use objc2_app_kit::{NSApplication, NSModalPanelRunLoopMode};
        use objc2_foundation::{NSRunLoop, NSTimer};
        use std::ptr::NonNull;
        // Debug-only smoke harness: abort the modal sheet from the main run
        // loop so automated checks can prove the native sheet presents.
        let block = RcBlock::new(move |_timer: NonNull<NSTimer>| {
            if let Some(mtm) = TimerMarker::new() {
                NSApplication::sharedApplication(mtm).abortModal();
            }
        });
        let timer =
            NSTimer::timerWithTimeInterval_repeats_block(delay.as_secs_f64(), false, &block);
        NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSModalPanelRunLoopMode);
    }

    let response = alert.runModal();
    if response != NSAlertFirstButtonReturn {
        // Clear native field contents on cancel as well.
        for field in &fields {
            field.setStringValue(&NSString::from_str(""));
        }
        return Ok(None);
    }

    let mut values = Vec::with_capacity(fields.len());
    for field in &fields {
        let value = NSControl::stringValue(field).to_string();
        field.setStringValue(&NSString::from_str(""));
        values.push(value);
    }
    Ok(Some(values))
}

/// Debug-only native sheet smoke harness used by `scripts/native-sheet-smoke.sh`.
/// It presents the sheet and aborts it after a short delay; it cannot enter a
/// passphrase and therefore does not replace the human smoke test.
#[cfg(all(target_os = "macos", debug_assertions))]
pub fn smoke_test<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> AppResult<()> {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = unsafe {
            present_prompt(
                spec(SecurePurpose::Unlock),
                Some(Duration::from_millis(1500)),
            )
        };
        let _ = tx.send(result);
    })
    .map_err(|_| AppError::NativeInput)?;
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(None)) => Ok(()),
        _ => Err(AppError::NativeInput),
    }
}

#[cfg(not(target_os = "macos"))]
fn show_native<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
    _spec: PromptSpec,
) -> AppResult<Option<Vec<SecretString>>> {
    Err(AppError::NativeInput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_prompt_has_fixed_rust_owned_text() {
        for purpose in [
            SecurePurpose::CreateVault,
            SecurePurpose::Unlock,
            SecurePurpose::ChangeCurrent,
            SecurePurpose::ChangeNew,
            SecurePurpose::RestoreBackup,
            SecurePurpose::RestoreCurrentVault,
            SecurePurpose::RestoreChangeCurrentNew,
            SecurePurpose::BackupPassphrase,
            SecurePurpose::BackupUnlock,
            SecurePurpose::PlaidSetup,
        ] {
            let spec = spec(purpose);
            assert!(!spec.title.is_empty());
            assert!(!spec.message.is_empty());
            assert!(!spec.labels.is_empty());
            assert!(!spec.action.is_empty());
            assert_eq!(spec.cancel, "Cancel");
        }
    }
}
