//! Launch at login through `SMAppService`. macOS owns this state, so it is
//! never copied into settings. It only works from a signed app installed in
//! `/Applications`; `cargo run` builds report an error instead.

use color_eyre::eyre::{Result, eyre};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginItemStatus {
    Disabled,
    Enabled,
    /// Registered, but the user must approve it in System Settings.
    RequiresApproval,
}

/// These calls block on a system service; run them off the UI thread.
pub fn status() -> Result<LoginItemStatus> {
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: mainAppService and status do not retain caller-owned pointers.
        let status = unsafe { SMAppService::mainAppService().status() };
        match status {
            SMAppServiceStatus::NotRegistered | SMAppServiceStatus::NotFound => {
                Ok(LoginItemStatus::Disabled)
            }
            SMAppServiceStatus::Enabled => Ok(LoginItemStatus::Enabled),
            SMAppServiceStatus::RequiresApproval => Ok(LoginItemStatus::RequiresApproval),
            _ => Err(eyre!("macOS returned an unknown login item status")),
        }
    })
}

pub fn set_enabled(enabled: bool) -> Result<LoginItemStatus> {
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: SMAppService owns the NSError produced by these synchronous APIs.
        let service = unsafe { SMAppService::mainAppService() };
        let current = status()?;
        if enabled && current == LoginItemStatus::Disabled {
            unsafe { service.registerAndReturnError() }
                .map_err(|error| eyre!(error.to_string()))?;
        } else if !enabled && current != LoginItemStatus::Disabled {
            unsafe { service.unregisterAndReturnError() }
                .map_err(|error| eyre!(error.to_string()))?;
        }
        status()
    })
}

pub fn open_system_settings() {
    // SAFETY: This only asks macOS to open its Login Items settings pane.
    unsafe { SMAppService::openSystemSettingsLoginItems() };
}
