//! Supervises llama-server: start, wait for health, restart on crash, stop.
//! Must not read settings or pick the model; the caller passes every path.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use url::Url;

use crate::server_process::{is_serving, pick_free_port, spawn_server};

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(250);
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const CRASH_WINDOW: Duration = Duration::from_secs(60);
const MAX_RESTARTS_IN_WINDOW: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub server_binary: PathBuf,
    pub model_file: PathBuf,
    pub mmproj_file: PathBuf,
    pub context_tokens: u32,
    /// llama-server's stdout and stderr are appended here. Its folder must exist.
    pub log_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeState {
    Starting,
    /// `base_url` is the OpenAI-compatible root, `http://127.0.0.1:<port>/v1`.
    Ready {
        base_url: Url,
    },
    Down {
        reason: String,
    },
}

pub struct Runtime {
    api_key: String,
    state: watch::Receiver<RuntimeState>,
    stop_signal: oneshot::Sender<()>,
    supervisor: JoinHandle<()>,
}

impl Runtime {
    /// Starts supervising in the background. Must be called inside a tokio runtime.
    pub fn start(config: RuntimeConfig) -> Runtime {
        let api_key = generate_api_key();
        let (state_sender, state) = watch::channel(RuntimeState::Starting);
        let (stop_signal, stop_requested) = oneshot::channel();
        let supervisor = tokio::spawn(supervise(
            config,
            api_key.clone(),
            state_sender,
            stop_requested,
        ));
        Runtime {
            api_key,
            state,
            stop_signal,
            supervisor,
        }
    }

    /// The bearer token every request to this server must carry. New for each `start`.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn watch_state(&self) -> watch::Receiver<RuntimeState> {
        self.state.clone()
    }

    /// Kills llama-server and waits until it has exited.
    pub async fn stop(self) {
        let _ = self.stop_signal.send(());
        let _ = self.supervisor.await;
    }
}

fn generate_api_key() -> String {
    let mut key_bytes = [0u8; 32];
    getrandom::fill(&mut key_bytes).expect("the OS random number generator is unavailable");
    hex::encode(key_bytes)
}

async fn supervise(
    config: RuntimeConfig,
    api_key: String,
    state: watch::Sender<RuntimeState>,
    mut stop_requested: oneshot::Receiver<()>,
) {
    // reqwest follows the system and env proxy by default, which cannot reach this Mac's loopback.
    let health_client = reqwest::Client::builder()
        .timeout(HEALTH_REQUEST_TIMEOUT)
        .no_proxy()
        .build()
        .expect("a client with only a timeout and no proxy always builds");
    let mut recent_crashes: VecDeque<Instant> = VecDeque::new();
    loop {
        state.send_replace(RuntimeState::Starting);
        let launch =
            pick_free_port().and_then(|port| Ok((port, spawn_server(&config, port, &api_key)?)));
        let (port, mut server) = match launch {
            Ok(launched) => launched,
            Err(error) => {
                state.send_replace(RuntimeState::Down {
                    reason: format!(
                        "could not start {}: {error}",
                        config.server_binary.display()
                    ),
                });
                return;
            }
        };

        let mut announced_ready = false;
        let mut health_poll = tokio::time::interval(HEALTH_POLL_INTERVAL);
        let exit = loop {
            tokio::select! {
                // Also fires when the Runtime is dropped without `stop`, so the server never outlives it.
                _ = &mut stop_requested => {
                    let _ = server.kill().await;
                    return;
                }
                exit = server.wait() => break exit,
                _ = health_poll.tick(), if !announced_ready => {
                    if is_serving(&health_client, port).await {
                        let base_url = Url::parse(&format!("http://127.0.0.1:{port}/v1"))
                            .expect("a loopback URL with a port always parses");
                        state.send_replace(RuntimeState::Ready { base_url });
                        announced_ready = true;
                    }
                }
            }
        };

        let now = Instant::now();
        recent_crashes.push_back(now);
        recent_crashes.retain(|crashed_at| now.duration_since(*crashed_at) < CRASH_WINDOW);
        if recent_crashes.len() > MAX_RESTARTS_IN_WINDOW {
            let exit = exit.map_or_else(|error| error.to_string(), |status| status.to_string());
            state.send_replace(RuntimeState::Down {
                reason: format!(
                    "llama-server stopped {} times within a minute (last: {exit})",
                    recent_crashes.len()
                ),
            });
            return;
        }
    }
}
