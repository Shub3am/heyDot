//! Says where an installed chat model's files are.
//! Must not download, hash or delete anything: a file under its final name is already verified.

use std::path::{Path, PathBuf};

use crate::catalog::Model;

const PROJECTOR_FILE_PREFIX: &str = "mmproj-";

/// The two files llama-server loads for a vision chat model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatModelFiles {
    pub weights: PathBuf,
    pub projector: PathBuf,
}

pub(crate) fn model_dir(model: &Model, models_dir: &Path) -> PathBuf {
    models_dir.join(model.id)
}

/// `None` while any file of `model` is missing, and for models without exactly one weights
/// file and one `mmproj-` projector file (speech and VAD models).
pub fn installed_chat_model(model: &Model, models_dir: &Path) -> Option<ChatModelFiles> {
    let dir = model_dir(model, models_dir);
    if !model.files.iter().all(|file| dir.join(file.name).is_file()) {
        return None;
    }
    let (projectors, weights): (Vec<_>, Vec<_>) = model
        .files
        .iter()
        .partition(|file| file.name.starts_with(PROJECTOR_FILE_PREFIX));
    match (weights.as_slice(), projectors.as_slice()) {
        ([weights], [projector]) => Some(ChatModelFiles {
            weights: dir.join(weights.name),
            projector: dir.join(projector.name),
        }),
        _ => None,
    }
}
