//! One conversation with the model: the turns it remembers and the one answer in flight.
//! Must not capture the screen, speak, or know about Tauri, the UI or which model serves the answer.

mod context;

pub use context::UserInput;
