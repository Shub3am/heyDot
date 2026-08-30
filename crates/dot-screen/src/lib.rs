//! Captures the display the user is looking at as a JPEG sized for a vision model.
//! Must not know about models, providers, settings files or the UI; the caller passes the size limit.

mod capture;
mod jpeg;
mod scale;
mod screen_error;

pub use capture::{capture_display_under_cursor, request_screen_access};
pub use screen_error::ScreenError;
