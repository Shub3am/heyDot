use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dot_runtime::{Runtime, RuntimeConfig, RuntimeState};
use tempfile::TempDir;

const WAIT_LIMIT: Duration = Duration::from_secs(10);

fn config_for(folder: &Path, fake_mode: &str) -> RuntimeConfig {
    let model_file = folder.join("model.gguf");
    fs::write(&model_file, fake_mode).unwrap();
    RuntimeConfig {
        server_binary: PathBuf::from(env!("CARGO_BIN_EXE_fake-llama-server")),
        model_file,
        mmproj_file: folder.join("mmproj.gguf"),
        context_tokens: 8192,
        log_file: folder.join("llama-server.log"),
    }
}

async fn wait_for_state(runtime: &Runtime, wanted: impl Fn(&RuntimeState) -> bool) -> RuntimeState {
    let mut states = runtime.watch_state();
    let state = tokio::time::timeout(WAIT_LIMIT, states.wait_for(|state| wanted(state)))
        .await
        .expect("timed out waiting for the runtime state")
        .expect("the supervisor stopped before reaching the state");
    state.clone()
}

async fn wait_until_ready(runtime: &Runtime) -> u16 {
    match wait_for_state(runtime, |state| matches!(state, RuntimeState::Ready { .. })).await {
        RuntimeState::Ready { base_url } => base_url.port().unwrap(),
        _ => unreachable!(),
    }
}

async fn wait_until_down(runtime: &Runtime) -> String {
    match wait_for_state(runtime, |state| matches!(state, RuntimeState::Down { .. })).await {
        RuntimeState::Down { reason } => reason,
        _ => unreachable!(),
    }
}

fn recorded(folder: &Path, extension: &str) -> String {
    fs::read_to_string(folder.join("model").with_extension(extension)).unwrap_or_default()
}

fn launch_count(folder: &Path) -> usize {
    recorded(folder, "launches").lines().count()
}

fn port_accepts_connections(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

#[tokio::test]
async fn starts_in_the_starting_state() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "loading"));

    assert_eq!(*runtime.watch_state().borrow(), RuntimeState::Starting);
    runtime.stop().await;
}

#[tokio::test]
async fn becomes_ready_with_a_loopback_v1_base_url_once_health_answers_200() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "ready"));

    let port = wait_until_ready(&runtime).await;

    let expected = format!("http://127.0.0.1:{port}/v1");
    assert_eq!(
        *runtime.watch_state().borrow(),
        RuntimeState::Ready {
            base_url: expected.parse().unwrap()
        }
    );
    runtime.stop().await;
}

#[tokio::test]
async fn passes_model_files_context_and_loopback_host_on_the_command_line() {
    let folder = TempDir::new().unwrap();
    let config = config_for(folder.path(), "ready");
    let runtime = Runtime::start(config.clone());

    let port = wait_until_ready(&runtime).await;

    let expected_args = [
        "-m".to_string(),
        config.model_file.display().to_string(),
        "--mmproj".to_string(),
        config.mmproj_file.display().to_string(),
        "-c".to_string(),
        "8192".to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        port.to_string(),
        "--no-ui".to_string(),
        "--jinja".to_string(),
    ]
    .join("\n");
    assert_eq!(recorded(folder.path(), "args"), expected_args);
    runtime.stop().await;
}

#[tokio::test]
async fn hands_the_api_key_to_the_server_through_the_environment_only() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "ready"));

    wait_until_ready(&runtime).await;

    assert_eq!(runtime.api_key().len(), 64);
    assert_eq!(recorded(folder.path(), "api_key"), runtime.api_key());
    assert!(!recorded(folder.path(), "args").contains(runtime.api_key()));
    runtime.stop().await;
}

#[tokio::test]
async fn every_start_gets_a_different_api_key() {
    let first_folder = TempDir::new().unwrap();
    let second_folder = TempDir::new().unwrap();
    let first = Runtime::start(config_for(first_folder.path(), "loading"));
    let second = Runtime::start(config_for(second_folder.path(), "loading"));

    assert_ne!(first.api_key(), second.api_key());
    first.stop().await;
    second.stop().await;
}

#[tokio::test]
async fn restarts_a_crashed_server_and_becomes_ready() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "crash-first-launch"));

    wait_until_ready(&runtime).await;

    assert_eq!(launch_count(folder.path()), 2);
    runtime.stop().await;
}

#[tokio::test]
async fn gives_up_after_the_fourth_crash_within_a_minute() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "crash"));

    let reason = wait_until_down(&runtime).await;

    assert_eq!(launch_count(folder.path()), 4);
    assert!(reason.contains("4 times"), "reason was: {reason}");
    runtime.stop().await;
}

#[tokio::test]
async fn missing_server_binary_is_down_with_the_path_in_the_reason() {
    let folder = TempDir::new().unwrap();
    let mut config = config_for(folder.path(), "ready");
    config.server_binary = folder.path().join("no-such-llama-server");
    let runtime = Runtime::start(config);

    let reason = wait_until_down(&runtime).await;

    assert!(
        reason.contains("no-such-llama-server"),
        "reason was: {reason}"
    );
    runtime.stop().await;
}

#[tokio::test]
async fn stop_kills_a_ready_server() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "ready"));
    let port = wait_until_ready(&runtime).await;
    assert!(port_accepts_connections(port));

    runtime.stop().await;

    assert!(!port_accepts_connections(port));
}

#[tokio::test]
async fn stop_kills_a_server_that_is_still_loading() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "loading"));
    let port = tokio::time::timeout(WAIT_LIMIT, async {
        loop {
            if let Some(port) = recorded(folder.path(), "args")
                .lines()
                .skip_while(|arg| *arg != "--port")
                .nth(1)
            {
                let port: u16 = port.parse().unwrap();
                if port_accepts_connections(port) {
                    return port;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the fake server never started listening");

    runtime.stop().await;

    assert!(!port_accepts_connections(port));
}
