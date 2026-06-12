//! Screen Recording TCC preflight. xcap silently returns wallpaper-only
//! frames without the grant, so every scroll session is gated on this.

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
pub fn has_screen_recording() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
pub fn request_screen_recording() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(not(target_os = "macos"))]
pub fn has_screen_recording() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn request_screen_recording() -> bool {
    true
}
