use std::fs;
use std::path::Path;

use dot_models::{ChatModelFiles, QWEN3_VL_4B, SILERO_VAD_V6, installed_chat_model};
use tempfile::TempDir;

const WEIGHTS: &str = "Qwen3VL-4B-Instruct-Q4_K_M.gguf";
const PROJECTOR: &str = "mmproj-Qwen3VL-4B-Instruct-F16.gguf";

fn put_file(models_dir: &Path, model_id: &str, file_name: &str) {
    let model_dir = models_dir.join(model_id);
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join(file_name), b"x").unwrap();
}

#[test]
fn installed_chat_model_returns_weights_and_projector_paths() {
    let models_dir = TempDir::new().unwrap();
    put_file(models_dir.path(), QWEN3_VL_4B.id, WEIGHTS);
    put_file(models_dir.path(), QWEN3_VL_4B.id, PROJECTOR);

    let model_dir = models_dir.path().join(QWEN3_VL_4B.id);
    assert_eq!(
        installed_chat_model(&QWEN3_VL_4B, models_dir.path()),
        Some(ChatModelFiles {
            weights: model_dir.join(WEIGHTS),
            projector: model_dir.join(PROJECTOR),
        })
    );
}

#[test]
fn nothing_downloaded_is_not_installed() {
    let models_dir = TempDir::new().unwrap();

    assert_eq!(installed_chat_model(&QWEN3_VL_4B, models_dir.path()), None);
}

#[test]
fn a_projector_still_partial_is_not_installed() {
    let models_dir = TempDir::new().unwrap();
    put_file(models_dir.path(), QWEN3_VL_4B.id, WEIGHTS);
    put_file(
        models_dir.path(),
        QWEN3_VL_4B.id,
        &format!("{PROJECTOR}.part"),
    );

    assert_eq!(installed_chat_model(&QWEN3_VL_4B, models_dir.path()), None);
}

#[test]
fn a_model_without_a_projector_is_not_a_chat_model() {
    let models_dir = TempDir::new().unwrap();
    for file in SILERO_VAD_V6.files {
        put_file(models_dir.path(), SILERO_VAD_V6.id, file.name);
    }

    assert_eq!(
        installed_chat_model(&SILERO_VAD_V6, models_dir.path()),
        None
    );
}
