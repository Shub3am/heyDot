//! Stands in for llama-server in dot-runtime's tests; never shipped or run by the app.
//! The model file's text picks the behaviour: `ready`, `loading`, `crash` or `crash-first-launch`.
//! Next to the model file it records the arguments, the API key and one line per launch.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

const HEALTH_CHECKS_BEFORE_READY: usize = 2;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value_after = |flag: &str| {
        let position = args
            .iter()
            .position(|arg| arg == flag)
            .expect("flag missing");
        args[position + 1].clone()
    };
    let model_file = PathBuf::from(value_after("-m"));
    let port = value_after("--port");
    let recording = |extension: &str| model_file.with_extension(extension);

    fs::write(recording("args"), args.join("\n")).unwrap();
    fs::write(
        recording("api_key"),
        std::env::var("LLAMA_API_KEY").unwrap_or_default(),
    )
    .unwrap();
    let mut launches = OpenOptions::new()
        .create(true)
        .append(true)
        .open(recording("launches"))
        .unwrap();
    writeln!(launches, "launch").unwrap();
    let launch_count = fs::read_to_string(recording("launches"))
        .unwrap()
        .lines()
        .count();

    let mode = fs::read_to_string(&model_file).unwrap();
    let health_checks_before_ready = match (mode.trim(), launch_count) {
        ("crash", _) | ("crash-first-launch", 1) => std::process::exit(1),
        ("loading", _) => usize::MAX,
        _ => HEALTH_CHECKS_BEFORE_READY,
    };

    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
    for (served, connection) in listener.incoming().enumerate() {
        let mut connection = connection.unwrap();
        let mut request = [0u8; 4096];
        let _ = connection.read(&mut request);
        let status_line = if served < health_checks_before_ready {
            "503 Service Unavailable"
        } else {
            "200 OK"
        };
        let body = r#"{"status":"ok"}"#;
        let _ = write!(
            connection,
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    }
}
