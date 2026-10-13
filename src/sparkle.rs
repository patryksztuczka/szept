//! Sparkle auto-updates. The framework is loaded at runtime from the app
//! bundle, so `cargo run`, tests, and builds packaged without an update feed
//! simply report updates as unavailable.

use std::cell::RefCell;
use std::path::PathBuf;

use color_eyre::eyre::{Result, WrapErr, eyre};
use libloading::Library;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::{MainThreadMarker, msg_send};

thread_local! {
    static UPDATER: RefCell<Option<Updater>> = const { RefCell::new(None) };
}

struct Updater {
    controller: Retained<AnyObject>,
    _framework: Library,
}

/// Starts Sparkle's standard updater, which runs scheduled background checks
/// according to `SUEnableAutomaticChecks` in Info.plist. Call once on the main
/// thread after the application has launched.
pub fn start() {
    if let Err(error) = start_inner() {
        tracing::error!("could not start Sparkle updater: {error:#}");
    }
}

fn start_inner() -> Result<()> {
    MainThreadMarker::new().ok_or_else(|| eyre!("Sparkle must start on the main thread"))?;
    if is_available() {
        return Ok(());
    }
    let Some(path) = framework_path() else {
        tracing::info!("Sparkle.framework is not bundled; updates are unavailable");
        return Ok(());
    };
    // Loading the framework registers its Objective-C classes.
    let framework = unsafe { Library::new(&path) }
        .wrap_err_with(|| format!("could not load {}", path.display()))?;
    let class = AnyClass::get(c"SPUStandardUpdaterController")
        .ok_or_else(|| eyre!("SPUStandardUpdaterController is unavailable"))?;
    let controller = unsafe {
        let allocated: *mut AnyObject = msg_send![class, alloc];
        let controller: *mut AnyObject = msg_send![
            allocated,
            initWithStartingUpdater: Bool::YES,
            updaterDelegate: std::ptr::null_mut::<AnyObject>(),
            userDriverDelegate: std::ptr::null_mut::<AnyObject>()
        ];
        Retained::from_raw(controller)
            .ok_or_else(|| eyre!("Sparkle updater initialization returned null"))?
    };
    UPDATER.with(|updater| {
        *updater.borrow_mut() = Some(Updater {
            controller,
            _framework: framework,
        });
    });
    Ok(())
}

pub fn is_available() -> bool {
    UPDATER.with(|updater| updater.borrow().is_some())
}

/// Shows Sparkle's "Check for Updates" UI. Activate the app first so the
/// dialog appears in front.
pub fn check_for_updates() {
    if MainThreadMarker::new().is_none() {
        tracing::warn!("Sparkle update checks must run on the main thread");
        return;
    }
    UPDATER.with(|updater| match updater.borrow().as_ref() {
        Some(updater) => unsafe {
            let _: () =
                msg_send![&*updater.controller, checkForUpdates: std::ptr::null_mut::<AnyObject>()];
        },
        None => tracing::warn!("Sparkle updater is not available"),
    });
}

fn framework_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let path = executable
        .parent()?
        .join("../Frameworks/Sparkle.framework/Versions/B/Sparkle");
    path.exists().then_some(path)
}
