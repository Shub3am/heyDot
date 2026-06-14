//! Proves the pinned llama-server runs the real Qwen3-VL 4B GGUF and answers through dot-providers.
//! Ignored by default: it needs `scripts/build-llama-server.sh` run and the model downloaded
//! (the app's Download button puts it where this test looks).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use dot_models::{QWEN3_VL_4B, installed_chat_model};
use dot_providers::{ChatConfig, ChatMessage, ChatRole, stream_chat};
use dot_runtime::{Runtime, RuntimeConfig, RuntimeState};
use futures_util::StreamExt;

const MODEL_LOAD_TIMEOUT: Duration = Duration::from_secs(120);

#[tokio::test]
#[ignore = "needs the built llama-server and the downloaded Qwen3-VL 4B model"]
async fn the_real_4b_model_answers_a_typed_question() {
    let models_dir = std::env::home_dir()
        .unwrap()
        .join("Library/Application Support/Hey Dot/models");
    let files = installed_chat_model(&QWEN3_VL_4B, &models_dir)
        .expect("download Qwen3-VL 4B with the app first");
    let log_folder = tempfile::tempdir().unwrap();
    let log_file = log_folder.path().join("llama-server.log");
    let started_at = Instant::now();
    let runtime = Runtime::start(RuntimeConfig {
        server_binary: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/src-tauri/binaries/llama-server-aarch64-apple-darwin"),
        model_file: files.weights,
        mmproj_file: files.projector,
        context_tokens: 8192,
        log_file: log_file.clone(),
    });

    let mut state = runtime.watch_state();
    let settled = tokio::time::timeout(
        MODEL_LOAD_TIMEOUT,
        state.wait_for(|state| *state != RuntimeState::Starting),
    )
    .await
    .expect("llama-server did not load the model within two minutes")
    .unwrap()
    .clone();
    let RuntimeState::Ready { base_url } = settled else {
        panic!(
            "{settled:?}\n{}",
            std::fs::read_to_string(&log_file).unwrap_or_default()
        );
    };
    eprintln!("ready after {:?}", started_at.elapsed());

    let config = ChatConfig {
        base_url,
        api_key: runtime.api_key().to_owned(),
        model: QWEN3_VL_4B.id.to_owned(),
    };
    let question = [ChatMessage {
        role: ChatRole::User,
        text: "What is the capital of France? Answer in one word.".to_owned(),
        jpeg_image: None,
    }];
    let asked_at = Instant::now();
    let http = reqwest::Client::new();
    let mut answer_stream = std::pin::pin!(stream_chat(&http, &config, &question));
    let mut answer = String::new();
    while let Some(delta) = answer_stream.next().await {
        if answer.is_empty() {
            eprintln!("first text after {:?}", asked_at.elapsed());
        }
        answer.push_str(&delta.unwrap());
    }
    eprintln!("whole answer after {:?}: {answer:?}", asked_at.elapsed());
    runtime.stop().await;

    assert!(answer.contains("Paris"), "{answer}");
}
