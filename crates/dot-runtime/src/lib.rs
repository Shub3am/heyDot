//! Keeps one llama-server process running for a local model and says whether it can take requests.
//! Must not know about chat messages, providers, the UI or where Hey Dot keeps its files.

mod runtime;
mod server_process;

pub use runtime::{Runtime, RuntimeConfig, RuntimeState};
