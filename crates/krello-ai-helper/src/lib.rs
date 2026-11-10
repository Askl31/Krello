//! Embedded Swift helper binary (compiled by build.rs).
//!
//! `helper_binary()` returns the bytes of the helper compiled from
//! `swift/main.swift` (empty slice when not on macOS or compilation failed).

pub fn helper_binary() -> &'static [u8] {
    include_bytes!(env!("KRELLO_AI_HELPER_PATH"))
}

/// True when we are on macOS and a real helper was compiled and embedded.
pub fn apple_intelligence_supported_os() -> bool {
    cfg!(target_os = "macos") && !helper_binary().is_empty()
}
