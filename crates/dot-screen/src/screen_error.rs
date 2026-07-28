//! Every way a screenshot can fail, with a missing permission kept apart so the app can ask for it.
//! Must not carry UI wording beyond one plain sentence per case.

use screencapturekit::error::SCError;

#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    #[error("Hey Dot does not have Screen Recording permission")]
    PermissionNotGranted,
    #[error("no display is under the mouse cursor")]
    NoDisplayUnderCursor,
    #[error("ScreenCaptureKit failed: {0}")]
    CaptureFailed(#[from] SCError),
    #[error("could not encode the screenshot as JPEG: {0}")]
    EncodeFailed(#[from] image::ImageError),
}
