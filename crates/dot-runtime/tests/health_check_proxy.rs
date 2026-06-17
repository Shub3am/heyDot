// Its own test binary because it sets HTTP_PROXY for the whole process.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use dot_runtime::{Runtime, RuntimeConfig, RuntimeState};
use tempfile::TempDir;

#[tokio::test]
async fn becomes_ready_when_an_unreachable_http_proxy_is_configured() {
    // SAFETY: this binary has one test and it sets the variables before any other thread starts.
    unsafe {
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:1");
        std::env::set_var("http_proxy", "http://127.0.0.1:1");
    }
    let folder = TempDir::new().unwrap();
    let model_file = folder.path().join("model.gguf");
    fs::write(&model_file, "ready").unwrap();
    let runtime = Runtime::start(RuntimeConfig {
        server_binary: PathBuf::from(env!("CARGO_BIN_EXE_fake-llama-server")),
        model_file,
        mmproj_file: folder.path().join("mmproj.gguf"),
        context_tokens: 8192,
        log_file: folder.path().join("llama-server.log"),
    });

    let mut states = runtime.watch_state();
    tokio::time::timeout(
        Duration::from_secs(10),
        states.wait_for(|state| matches!(state, RuntimeState::Ready { .. })),
    )
    .await
    .expect("health checks went to the proxy instead of the loopback server")
    .expect("the supervisor stopped before becoming ready");
}
