//! panic.rs: Panic handler.
//! Part of the Animats impostor system
//!
//! At this point, the asset exists on the SL/OS asset store.
//! A script running in an SL/OS viewer calls this service to tell it about new assets.
//!
//!     License: LGPL.
//!     Animats
//!     August, 2025.
//!
/// Catch panics and log.
/// Otherwise this info is lost in servers.
pub fn catch_panic() {
    // Set a custom panic hook
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.as_str()
        } else {
            "Unable to decode panic msg"
        };   
        log::error!("PANIC: {:?}: {}", info, msg);
    }));
}
