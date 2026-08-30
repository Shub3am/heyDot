//! Every way a screenshot can fail, with a missing permission kept apart so the app can ask for it.
//! Must not carry UI wording beyond one plain sentence per case.

#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    #[error("could not encode the screenshot as JPEG: {0}")]
    EncodeFailed(#[from] image::ImageError),
}
