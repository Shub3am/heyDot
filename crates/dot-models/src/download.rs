//! Gets a catalog model's files onto disk, resuming partial files and only ever giving a
//! file its final name after its sha256 matched.
//! Must not choose which model to download or where the models folder is.

use std::io::Read;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header::RANGE;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::catalog::{Model, ModelFile};

/// Progress across all of a model's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("download request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("server answered {status} for {url}")]
    UnexpectedStatus { url: String, status: u16 },
    #[error("could not write the model file: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "{file_name} failed verification (expected sha256 {expected}, got {actual}); the partial file was deleted"
    )]
    HashMismatch {
        file_name: String,
        expected: String,
        actual: String,
    },
}

/// Downloads every file of `model` into `<models_dir>/<model.id>/` and returns that folder.
/// Dropping the future cancels the download and keeps the partial file for the next call.
pub async fn download_model(
    client: &reqwest::Client,
    model: &Model,
    models_dir: &Path,
    mut on_progress: impl FnMut(DownloadProgress),
) -> Result<PathBuf, DownloadError> {
    let model_dir = models_dir.join(model.id);
    tokio::fs::create_dir_all(&model_dir).await?;
    let total_bytes: u64 = model.files.iter().map(|file| file.bytes).sum();
    let mut finished_bytes = 0;
    for file in model.files {
        download_file(client, file, &model_dir, |file_bytes| {
            on_progress(DownloadProgress {
                downloaded_bytes: finished_bytes + file_bytes,
                total_bytes,
            })
        })
        .await?;
        finished_bytes += file.bytes;
        on_progress(DownloadProgress {
            downloaded_bytes: finished_bytes,
            total_bytes,
        });
    }
    Ok(model_dir)
}

async fn download_file(
    client: &reqwest::Client,
    file: &ModelFile,
    model_dir: &Path,
    mut on_file_progress: impl FnMut(u64),
) -> Result<(), DownloadError> {
    let final_path = model_dir.join(file.name);
    if tokio::fs::try_exists(&final_path).await? {
        return Ok(());
    }
    let partial_path = model_dir.join(format!("{}.part", file.name));
    let resume_from = match tokio::fs::metadata(&partial_path).await {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
    };
    // A partial file already at full size (killed before the rename) needs no request; asking
    // for `bytes=<size>-` would get a 416.
    if resume_from < file.bytes {
        fetch_into_partial_file(
            client,
            file,
            &partial_path,
            resume_from,
            &mut on_file_progress,
        )
        .await?;
    }
    let path_to_hash = partial_path.clone();
    let actual_sha256 = tokio::task::spawn_blocking(move || sha256_of_file(&path_to_hash))
        .await
        .expect("hashing task panicked")?;
    if actual_sha256 != file.sha256 {
        tokio::fs::remove_file(&partial_path).await?;
        return Err(DownloadError::HashMismatch {
            file_name: file.name.to_string(),
            expected: file.sha256.to_string(),
            actual: actual_sha256,
        });
    }
    tokio::fs::rename(&partial_path, &final_path).await?;
    Ok(())
}

async fn fetch_into_partial_file(
    client: &reqwest::Client,
    file: &ModelFile,
    partial_path: &Path,
    resume_from: u64,
    on_file_progress: &mut impl FnMut(u64),
) -> Result<(), DownloadError> {
    let mut request = client.get(file.url);
    if resume_from > 0 {
        request = request.header(RANGE, format!("bytes={resume_from}-"));
    }
    let response = request.send().await?;
    // 200 to a Range request means the server ignored it and is sending the whole file,
    // so the partial file must start over rather than be appended to.
    let mut written_bytes = match response.status() {
        StatusCode::PARTIAL_CONTENT => resume_from,
        StatusCode::OK => 0,
        status => {
            return Err(DownloadError::UnexpectedStatus {
                url: file.url.to_string(),
                status: status.as_u16(),
            });
        }
    };
    let mut partial_file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(written_bytes > 0)
        .truncate(written_bytes == 0)
        .open(partial_path)
        .await?;
    on_file_progress(written_bytes);
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        partial_file.write_all(&chunk).await?;
        written_bytes += chunk.len() as u64;
        on_file_progress(written_bytes);
    }
    // tokio hands writes to a background thread; flush waits for them and reports their errors,
    // which tokio's sync_all would swallow. sync_all then puts the bytes on disk (F_FULLFSYNC on
    // macOS) so a power loss after the rename cannot leave an installed file with lost data.
    partial_file.flush().await?;
    partial_file.sync_all().await?;
    Ok(())
}

fn sha256_of_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read_bytes = file.read(&mut buffer)?;
        if read_bytes == 0 {
            break;
        }
        hasher.update(&buffer[..read_bytes]);
    }
    Ok(hex::encode(hasher.finalize()))
}
