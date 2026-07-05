use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::Request;
use axum::http::header;
use axum::middleware::{self, Next};
use axum::routing::get;
use dot_models::{
    DownloadError, DownloadProgress, Model, ModelFile, SILERO_VAD_V6, download_model,
};
use sha2::{Digest, Sha256};
use tower_http::services::ServeDir;

struct FileServer {
    base_url: String,
    range_headers: Arc<Mutex<Vec<Option<String>>>>,
}

impl FileServer {
    /// The Range header of every request so far, in order; `None` means no Range header.
    fn range_headers_seen(&self) -> Vec<Option<String>> {
        self.range_headers.lock().unwrap().clone()
    }
}

/// Serves `served_dir` with Range support, plus `/ignores-range/<name>` which always answers
/// 200 with `ignores_range_body`, like a server without Range support.
async fn start_file_server(served_dir: &Path, ignores_range_body: Vec<u8>) -> FileServer {
    let range_headers = Arc::new(Mutex::new(Vec::new()));
    let recorded_range_headers = range_headers.clone();
    let app = Router::new()
        .route(
            "/ignores-range/{name}",
            get(move || {
                let body = ignores_range_body.clone();
                async move { body }
            }),
        )
        .fallback_service(ServeDir::new(served_dir))
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let recorded_range_headers = recorded_range_headers.clone();
            async move {
                let range = request
                    .headers()
                    .get(header::RANGE)
                    .map(|value| value.to_str().unwrap().to_string());
                recorded_range_headers.lock().unwrap().push(range);
                next.run(request).await
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FileServer {
        base_url,
        range_headers,
    }
}

/// Bytes that differ at every offset near each other, so a wrong resume offset corrupts the hash.
fn patterned_bytes(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}

fn sha256_hex(content: &[u8]) -> String {
    hex::encode(Sha256::digest(content))
}

// Catalog entries are `'static`; tests leak their few strings rather than widen the types.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn test_file(url: String, name: &'static str, content: &[u8]) -> ModelFile {
    ModelFile {
        name,
        url: leak(url),
        sha256: leak(sha256_hex(content)),
        bytes: content.len() as u64,
    }
}

fn test_model(files: Vec<ModelFile>) -> Model {
    Model {
        id: "test-model",
        display_name: "Test model",
        min_ram_gb: 0,
        files: Box::leak(files.into_boxed_slice()),
    }
}

#[tokio::test]
async fn fresh_download_writes_verified_files_and_reports_progress_to_the_total() {
    let served_dir = tempfile::tempdir().unwrap();
    let first = patterned_bytes(300_000);
    let second = patterned_bytes(70_000);
    std::fs::write(served_dir.path().join("first.bin"), &first).unwrap();
    std::fs::write(served_dir.path().join("second.bin"), &second).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![
        test_file(
            format!("{}/first.bin", server.base_url),
            "first.bin",
            &first,
        ),
        test_file(
            format!("{}/second.bin", server.base_url),
            "second.bin",
            &second,
        ),
    ]);
    let models_dir = tempfile::tempdir().unwrap();
    let mut progress_updates = Vec::new();

    let model_dir = download_model(
        &reqwest::Client::new(),
        &model,
        models_dir.path(),
        |update| progress_updates.push(update),
    )
    .await
    .unwrap();

    assert_eq!(model_dir, models_dir.path().join("test-model"));
    assert_eq!(std::fs::read(model_dir.join("first.bin")).unwrap(), first);
    assert_eq!(std::fs::read(model_dir.join("second.bin")).unwrap(), second);
    assert!(!model_dir.join("first.bin.part").exists());
    assert!(!model_dir.join("second.bin.part").exists());
    assert!(
        progress_updates
            .windows(2)
            .all(|pair| pair[0].downloaded_bytes <= pair[1].downloaded_bytes)
    );
    assert_eq!(
        progress_updates.last(),
        Some(&DownloadProgress {
            downloaded_bytes: 370_000,
            total_bytes: 370_000
        })
    );
    assert_eq!(server.range_headers_seen(), vec![None, None]);
}

#[tokio::test]
async fn interrupted_download_resumes_with_a_range_request() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(200_000);
    std::fs::write(served_dir.path().join("model.bin"), &content).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), &content[..120_000]).unwrap();

    download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert_eq!(
        server.range_headers_seen(),
        vec![Some("bytes=120000-".to_string())]
    );
}

#[tokio::test]
async fn server_ignoring_range_restarts_the_file_instead_of_appending() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(50_000);
    let server = start_file_server(served_dir.path(), content.clone()).await;
    let model = test_model(vec![test_file(
        format!("{}/ignores-range/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), vec![0xAA; 10_000]).unwrap();

    download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert_eq!(
        server.range_headers_seen(),
        vec![Some("bytes=10000-".to_string())]
    );
}

#[tokio::test]
async fn hash_mismatch_deletes_the_partial_file_and_reports_it() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(40_000);
    std::fs::write(served_dir.path().join("model.bin"), &content).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![ModelFile {
        sha256: leak(sha256_hex(b"different bytes")),
        ..test_file(
            format!("{}/model.bin", server.base_url),
            "model.bin",
            &content,
        )
    }]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");

    let error = download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        DownloadError::HashMismatch { ref file_name, ref actual, .. }
            if file_name == "model.bin" && *actual == sha256_hex(&content)
    ));
    assert!(!model_dir.join("model.bin.part").exists());
    assert!(!model_dir.join("model.bin").exists());
}

#[tokio::test]
async fn stale_partial_file_fails_verification_then_the_retry_succeeds() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(60_000);
    std::fs::write(served_dir.path().join("model.bin"), &content).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), vec![0x55; 20_000]).unwrap();
    let client = reqwest::Client::new();

    let first_attempt = download_model(&client, &model, models_dir.path(), |_| {}).await;
    let second_attempt = download_model(&client, &model, models_dir.path(), |_| {}).await;

    assert!(matches!(
        first_attempt,
        Err(DownloadError::HashMismatch { .. })
    ));
    assert!(second_attempt.is_ok());
    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert_eq!(
        server.range_headers_seen(),
        vec![Some("bytes=20000-".to_string()), None]
    );
}

#[tokio::test]
async fn complete_partial_file_is_verified_and_installed_without_a_request() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(30_000);
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), &content).unwrap();

    download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert!(!model_dir.join("model.bin.part").exists());
    assert_eq!(server.range_headers_seen(), Vec::<Option<String>>::new());
}

#[tokio::test]
async fn installed_file_is_not_downloaded_again() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(30_000);
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin"), &content).unwrap();
    let mut progress_updates = Vec::new();

    download_model(
        &reqwest::Client::new(),
        &model,
        models_dir.path(),
        |update| progress_updates.push(update),
    )
    .await
    .unwrap();

    assert_eq!(server.range_headers_seen(), Vec::<Option<String>>::new());
    assert_eq!(
        progress_updates.last(),
        Some(&DownloadProgress {
            downloaded_bytes: 30_000,
            total_bytes: 30_000
        })
    );
}

#[tokio::test]
async fn missing_file_on_the_server_is_a_status_error() {
    let served_dir = tempfile::tempdir().unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/missing.bin", server.base_url),
        "missing.bin",
        b"never served",
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");

    let error = download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        DownloadError::UnexpectedStatus { status: 404, .. }
    ));
    assert!(!model_dir.join("missing.bin.part").exists());
    assert!(!model_dir.join("missing.bin").exists());
}

#[tokio::test]
#[ignore = "network: downloads Silero VAD (2.3 MB) from GitHub and verifies its sha256"]
async fn silero_vad_downloads_and_verifies() {
    let models_dir = tempfile::tempdir().unwrap();

    let model_dir = download_model(
        &reqwest::Client::new(),
        &SILERO_VAD_V6,
        models_dir.path(),
        |_| {},
    )
    .await
    .unwrap();

    assert!(model_dir.join("silero_vad.onnx").exists());
}
