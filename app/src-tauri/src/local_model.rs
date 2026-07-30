//! Keeps the chosen local chat model usable: its files on disk and llama-server serving it.
//! Must not know about questions, answers or cloud providers.

use std::path::PathBuf;
use std::sync::Arc;

use dot_models::{
    DownloadProgress, Model, ModelChoice, Recommendation, download_model, installed_chat_model,
};
use dot_providers::ChatConfig;
use dot_runtime::{Runtime, RuntimeConfig, RuntimeState};
use serde::Serialize;
use tokio::sync::{Mutex, watch};

const CONTEXT_TOKENS: u32 = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelStatus {
    pub model_name: &'static str,
    pub download_bytes: u64,
    pub phase: LocalModelPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LocalModelPhase {
    NotInstalled,
    Downloading { percent: u8 },
    DownloadFailed { reason: String },
    Starting,
    Ready,
    Down { reason: String },
}

pub struct LocalModelPaths {
    pub models_dir: PathBuf,
    pub server_binary: PathBuf,
    pub log_file: PathBuf,
}

pub struct LocalModel {
    model: &'static Model,
    paths: LocalModelPaths,
    http: reqwest::Client,
    status: watch::Sender<LocalModelStatus>,
    runtime: Mutex<Option<Runtime>>,
    download_lock: Mutex<()>,
}

impl LocalModel {
    pub fn new(model: &'static Model, paths: LocalModelPaths, http: reqwest::Client) -> LocalModel {
        let phase = match installed_chat_model(model, &paths.models_dir) {
            Some(_) => LocalModelPhase::Starting,
            None => LocalModelPhase::NotInstalled,
        };
        let status = LocalModelStatus {
            model_name: model.display_name,
            download_bytes: model.files.iter().map(|file| file.bytes).sum(),
            phase,
        };
        LocalModel {
            model,
            paths,
            http,
            status: watch::Sender::new(status),
            runtime: Mutex::new(None),
            download_lock: Mutex::new(()),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<LocalModelStatus> {
        self.status.subscribe()
    }

    /// Starts llama-server unless the files are missing or it is already running.
    pub async fn start_if_installed(self: &Arc<Self>) {
        let Some(files) = installed_chat_model(self.model, &self.paths.models_dir) else {
            return;
        };
        let mut runtime_slot = self.runtime.lock().await;
        if runtime_slot.is_some() {
            return;
        }
        let runtime = Runtime::start(RuntimeConfig {
            server_binary: self.paths.server_binary.clone(),
            model_file: files.weights,
            mmproj_file: files.projector,
            context_tokens: CONTEXT_TOKENS,
            log_file: self.paths.log_file.clone(),
        });
        let mut runtime_state = runtime.watch_state();
        *runtime_slot = Some(runtime);
        let local_model = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let phase = match &*runtime_state.borrow_and_update() {
                    RuntimeState::Starting => LocalModelPhase::Starting,
                    RuntimeState::Ready { .. } => LocalModelPhase::Ready,
                    RuntimeState::Down { reason } => LocalModelPhase::Down {
                        reason: reason.clone(),
                    },
                };
                local_model
                    .status
                    .send_modify(|status| status.phase = phase);
                if runtime_state.changed().await.is_err() {
                    return;
                }
            }
        });
    }

    /// Downloads the model and starts it. A second call while one runs does nothing.
    pub async fn download(self: &Arc<Self>) {
        let Ok(_download_guard) = self.download_lock.try_lock() else {
            return;
        };
        self.status
            .send_modify(|status| status.phase = LocalModelPhase::Downloading { percent: 0 });
        let downloaded =
            download_model(&self.http, self.model, &self.paths.models_dir, |progress| {
                self.status
                    .send_if_modified(|status| set_download_percent(status, &progress));
            })
            .await;
        match downloaded {
            Ok(_) => self.start_if_installed().await,
            Err(error) => self.status.send_modify(|status| {
                status.phase = LocalModelPhase::DownloadFailed {
                    reason: error.to_string(),
                }
            }),
        }
    }

    /// Where to send a chat request, or `None` until llama-server is ready.
    pub async fn chat_config(&self) -> Option<ChatConfig> {
        let runtime_slot = self.runtime.lock().await;
        let runtime = runtime_slot.as_ref()?;
        let RuntimeState::Ready { base_url } = runtime.watch_state().borrow().clone() else {
            return None;
        };
        Some(ChatConfig {
            base_url,
            api_key: runtime.api_key().to_owned(),
            model: self.model.id.to_owned(),
        })
    }

    pub async fn shutdown(&self) {
        if let Some(runtime) = self.runtime.lock().await.take() {
            runtime.stop().await;
        }
    }
}

/// Returns whether the whole percent changed, so the UI hears at most 100 updates.
fn set_download_percent(status: &mut LocalModelStatus, progress: &DownloadProgress) -> bool {
    let percent = (progress.downloaded_bytes * 100 / progress.total_bytes.max(1)) as u8;
    let phase = LocalModelPhase::Downloading { percent };
    if status.phase == phase {
        return false;
    }
    status.phase = phase;
    true
}

/// The local model this Mac gets until onboarding lets the user choose (Phase 1 step 8).
pub fn pick_local_model(recommendation: &Recommendation) -> &'static Model {
    std::iter::once(&recommendation.recommended)
        .chain(&recommendation.also_offered)
        .find_map(|choice| match choice {
            ModelChoice::Local(model) => Some(*model),
            ModelChoice::Cloud => None,
        })
        .expect("every recommendation offers a local model")
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::time::Duration;

    use dot_models::{Chip, Hardware, ModelFile, QWEN3_VL_2B, QWEN3_VL_4B, recommend};

    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn local_model_downloading_from(url: String, models_dir: PathBuf) -> Arc<LocalModel> {
        let files = vec![ModelFile {
            name: "weights.gguf",
            url: url.leak(),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            bytes: 1000,
        }];
        let model = Box::leak(Box::new(Model {
            id: "test-model",
            display_name: "Test model",
            min_ram_gb: 0,
            files: files.leak(),
        }));
        let paths = LocalModelPaths {
            models_dir,
            server_binary: PathBuf::from("llama-server"),
            log_file: PathBuf::from("llama-server.log"),
        };
        Arc::new(LocalModel::new(model, paths, reqwest::Client::new()))
    }

    #[tokio::test]
    async fn a_model_that_is_not_running_has_no_chat_config() {
        let models_dir = tempfile::tempdir().unwrap();
        let local_model = local_model_downloading_from(
            "http://127.0.0.1:9/".to_owned(),
            models_dir.path().into(),
        );
        assert_eq!(
            local_model.subscribe().borrow().phase,
            LocalModelPhase::NotInstalled
        );
        assert_eq!(local_model.chat_config().await, None);
    }

    #[tokio::test]
    async fn a_second_download_while_one_runs_returns_at_once() {
        // Never accepted: the connection sits in the backlog, so the first download hangs.
        let silent_server = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/weights.gguf",
            silent_server.local_addr().unwrap()
        );
        let models_dir = tempfile::tempdir().unwrap();
        let local_model = local_model_downloading_from(url, models_dir.path().into());
        let first_download = tokio::spawn({
            let local_model = Arc::clone(&local_model);
            async move { local_model.download().await }
        });
        let mut status = local_model.subscribe();
        status
            .wait_for(|status| matches!(status.phase, LocalModelPhase::Downloading { .. }))
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(1), local_model.download())
            .await
            .expect("the second download waited for the first");
        assert_eq!(
            status.borrow().phase,
            LocalModelPhase::Downloading { percent: 0 }
        );
        first_download.abort();
    }

    fn downloading_status(percent: u8) -> LocalModelStatus {
        LocalModelStatus {
            model_name: "Test model",
            download_bytes: 1000,
            phase: LocalModelPhase::Downloading { percent },
        }
    }

    #[test]
    fn download_percent_changes_only_on_a_new_whole_percent() {
        let mut status = downloading_status(0);
        let at = |downloaded_bytes| DownloadProgress {
            downloaded_bytes,
            total_bytes: 1000,
        };
        assert!(!set_download_percent(&mut status, &at(9)));
        assert!(set_download_percent(&mut status, &at(10)));
        assert_eq!(status, downloading_status(1));
        assert!(!set_download_percent(&mut status, &at(19)));
        assert!(set_download_percent(&mut status, &at(1000)));
        assert_eq!(status, downloading_status(100));
    }

    #[test]
    fn intel_macs_get_the_2b_model_behind_the_cloud_recommendation() {
        let intel = Hardware {
            chip: Chip::Intel,
            ram_bytes: 16 * GIB,
        };
        assert_eq!(pick_local_model(&recommend(&intel)), &QWEN3_VL_2B);
    }

    #[test]
    fn a_16_gb_apple_silicon_mac_gets_the_4b_model() {
        let apple_silicon = Hardware {
            chip: Chip::AppleSilicon,
            ram_bytes: 16 * GIB,
        };
        assert_eq!(pick_local_model(&recommend(&apple_silicon)), &QWEN3_VL_4B);
    }

    #[test]
    fn status_serializes_in_the_shape_the_chat_panel_reads() {
        let status = LocalModelStatus {
            model_name: "Qwen3-VL 4B",
            download_bytes: 3,
            phase: LocalModelPhase::DownloadFailed {
                reason: "disk full".to_owned(),
            },
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({
                "modelName": "Qwen3-VL 4B",
                "downloadBytes": 3,
                "phase": {"kind": "downloadFailed", "reason": "disk full"}
            })
        );
    }
}
