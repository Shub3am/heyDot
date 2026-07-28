//! Launches one llama-server process and checks whether it is serving.
//! Must not restart, retry or decide anything; the supervisor in `runtime` does that.

use std::fs::OpenOptions;
use std::net::TcpListener;
use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::runtime::RuntimeConfig;

/// Asks the OS for a free port. The port can be taken again before llama-server binds it;
/// that shows up as a crash and the next restart picks a new port.
pub(crate) fn pick_free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

pub(crate) fn spawn_server(
    config: &RuntimeConfig,
    port: u16,
    api_key: &str,
) -> std::io::Result<Child> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.log_file)?;
    Command::new(&config.server_binary)
        .arg("-m")
        .arg(&config.model_file)
        .arg("--mmproj")
        .arg(&config.mmproj_file)
        .arg("-c")
        .arg(config.context_tokens.to_string())
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--no-ui",
            "--jinja",
        ])
        // Passed through the environment, not argv, so `ps` does not show it to other users.
        .env("LLAMA_API_KEY", api_key)
        .stdin(Stdio::null())
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .kill_on_drop(true)
        .spawn()
}

/// llama-server answers 200 on `/health` only once the model is loaded. While loading it
/// answers other codes or refuses the connection, which all mean "not ready yet".
pub(crate) async fn is_serving(client: &reqwest::Client, port: u16) -> bool {
    let response = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await;
    matches!(response, Ok(response) if response.status() == reqwest::StatusCode::OK)
}
