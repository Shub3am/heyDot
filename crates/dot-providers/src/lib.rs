//! Streams a chat answer from any OpenAI-compatible `/chat/completions` endpoint.
//! Must not start servers, pick models, hold conversation history or know which provider is behind the URL.

mod chat_request;
mod provider_error;
mod stream_chat;

pub use chat_request::{ChatConfig, ChatMessage, ChatRole};
pub use provider_error::ProviderError;
pub use stream_chat::stream_chat;
