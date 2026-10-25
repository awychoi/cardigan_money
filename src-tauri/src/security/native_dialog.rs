//! Native macOS confirmations. Destructive or lifecycle choices are presented
//! by AppKit; the renderer cannot supply or overlay their text.

use crate::error::{AppError, AppResult};
use crate::storage::settings::CloseBehavior;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    Hide,
    Quit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseChoice {
    pub action: CloseAction,
    pub remember: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    Hide,
    Quit,
    Ask,
}

pub fn decision(behavior: CloseBehavior) -> CloseDecision {
    match behavior {
        CloseBehavior::Hide => CloseDecision::Hide,
        CloseBehavior::Quit => CloseDecision::Quit,
        CloseBehavior::Unset => CloseDecision::Ask,
    }
}

/// First-close prompt: hide versus quit, with an explicit remember checkbox.
pub fn close_choice<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> AppResult<CloseChoice> {
    #[cfg(target_os = "macos")]
    {
        use std::sync::mpsc;
        let (tx, rx) = mpsc::sync_channel(1);
        app.run_on_main_thread(move || {
            let result = unsafe { present_close_alert() };
            let _ = tx.send(result);
        })
        .map_err(|_| AppError::NativeInput)?;
        match rx.recv_timeout(std::time::Duration::from_secs(300)) {
            Ok(Ok(choice)) => Ok(choice),
            Ok(Err(())) => Err(AppError::NativeInput),
            Err(_) => Err(AppError::NativeInput),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err(AppError::NativeInput)
    }
}

#[cfg(target_os = "macos")]
unsafe fn present_close_alert() -> Result<CloseChoice, ()> {
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSButton, NSButtonType,
        NSControlStateValueOff, NSControlStateValueOn, NSView, NSWindow, NSWindowSharingType,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

    let mtm = MainThreadMarker::new().ok_or(())?;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("Close cardigan_money?"));
    alert.setInformativeText(&NSString::from_str(
        "Hide keeps the app and scheduler running. Hiding is not a lock: the idle timer continues, and screen lock or sleep still locks the vault. Quit closes the database, zeroizes the session, and requires the passphrase next launch.",
    ));
    let _hide = alert.addButtonWithTitle(&NSString::from_str("Hide"));
    let _quit = alert.addButtonWithTitle(&NSString::from_str("Quit"));
    let _cancel = alert.addButtonWithTitle(&NSString::from_str("Cancel"));

    let container = NSView::initWithFrame(
        NSView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(380.0, 32.0)),
    );
    let remember: Retained<NSButton> = NSButton::new(mtm);
    remember.setButtonType(NSButtonType::Switch);
    remember.setTitle(&NSString::from_str(
        "Remember this choice (change it in Settings)",
    ));
    remember.setState(NSControlStateValueOff);
    remember.setFrame(NSRect::new(
        NSPoint::new(0.0, 4.0),
        NSSize::new(380.0, 24.0),
    ));
    container.addSubview(&remember);
    alert.setAccessoryView(Some(&container));
    let window: Retained<NSWindow> = alert.window();
    window.setSharingType(NSWindowSharingType::None);

    let response = alert.runModal();
    let action = if response == NSAlertFirstButtonReturn {
        CloseAction::Hide
    } else if response == NSAlertSecondButtonReturn {
        CloseAction::Quit
    } else {
        CloseAction::Cancel
    };
    let remember = remember.state() == NSControlStateValueOn;
    Ok(CloseChoice { action, remember })
}

/// Native confirmation for a destructive action. Returns true only when the
/// action button was chosen. There is no HTML fallback.
pub fn confirm_destructive<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    title: &'static str,
    message: &'static str,
    action: &'static str,
) -> AppResult<bool> {
    #[cfg(target_os = "macos")]
    {
        use std::sync::mpsc;
        let (tx, rx) = mpsc::sync_channel(1);
        app.run_on_main_thread(move || {
            let result = unsafe { present_confirm(title, message, action) };
            let _ = tx.send(result);
        })
        .map_err(|_| AppError::NativeInput)?;
        match rx.recv_timeout(std::time::Duration::from_secs(300)) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(())) => Err(AppError::NativeInput),
            Err(_) => Err(AppError::NativeInput),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, title, message, action);
        Err(AppError::NativeInput)
    }
}

#[cfg(target_os = "macos")]
unsafe fn present_confirm(
    title: &'static str,
    message: &'static str,
    action: &'static str,
) -> Result<bool, ()> {
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSWindow, NSWindowSharingType};
    use objc2_foundation::NSString;

    let mtm = MainThreadMarker::new().ok_or(())?;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    let _action = alert.addButtonWithTitle(&NSString::from_str(action));
    let _cancel = alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    let window: Retained<NSWindow> = alert.window();
    window.setSharingType(NSWindowSharingType::None);
    let response = alert.runModal();
    Ok(response == NSAlertFirstButtonReturn)
}

/// Post-restore acknowledgement. Returns true for "Change passphrase now" and
/// false for "Change later". Both choices leave the restored data in place.
pub fn restore_complete<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    message: &'static str,
) -> AppResult<bool> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        app.run_on_main_thread(move || {
            let result = unsafe { present_restore_complete(message) };
            let _ = tx.send(result);
        })
        .map_err(|_| AppError::NativeInput)?;
        match rx.recv_timeout(std::time::Duration::from_secs(300)) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(())) => Err(AppError::NativeInput),
            Err(_) => Err(AppError::NativeInput),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, message);
        Err(AppError::NativeInput)
    }
}

#[cfg(target_os = "macos")]
unsafe fn present_restore_complete(message: &'static str) -> Result<bool, ()> {
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSWindow, NSWindowSharingType};
    use objc2_foundation::NSString;

    let mtm = MainThreadMarker::new().ok_or(())?;
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("Restore complete"));
    alert.setInformativeText(&NSString::from_str(message));
    let _change = alert.addButtonWithTitle(&NSString::from_str("Change passphrase now"));
    let _later = alert.addButtonWithTitle(&NSString::from_str("Change later"));
    let window: Retained<NSWindow> = alert.window();
    window.setSharingType(NSWindowSharingType::None);
    let response = alert.runModal();
    Ok(response == NSAlertFirstButtonReturn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_decisions_follow_settings() {
        assert_eq!(decision(CloseBehavior::Hide), CloseDecision::Hide);
        assert_eq!(decision(CloseBehavior::Quit), CloseDecision::Quit);
        assert_eq!(decision(CloseBehavior::Unset), CloseDecision::Ask);
    }
}
