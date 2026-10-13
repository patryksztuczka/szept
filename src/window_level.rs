//! Keeps a window above other apps, including full-screen meeting apps.
//! GPUI only sets the window level when a window opens, so this reaches the
//! `NSWindow` behind a GPUI window directly.

use objc2::rc::Retained;
use objc2_app_kit::{
    NSFloatingWindowLevel, NSNormalWindowLevel, NSView, NSWindowCollectionBehavior,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Call on the main thread, e.g. from a GPUI window callback.
pub fn set_floating(window: &impl HasWindowHandle, floating: bool) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: GPUI hands out its live NSView, which stays valid for this call;
    // retaining it keeps it alive while we use it.
    let Some(view) = (unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }) else {
        return;
    };
    let Some(window) = view.window() else {
        return;
    };
    let mut behavior = window.collectionBehavior();
    let follow = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary;
    if floating {
        behavior |= follow;
    } else {
        behavior &= !follow;
    }
    window.setLevel(if floating {
        NSFloatingWindowLevel
    } else {
        NSNormalWindowLevel
    });
    window.setCollectionBehavior(behavior);
}
