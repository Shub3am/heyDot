//! What a chat request contains and its OpenAI wire format.
//! Must not send anything; `stream_chat` owns the HTTP call.

use base64::Engine;
use serde::Serialize;
use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatConfig {
    /// The OpenAI-compatible root, ending in `/v1` for llama-server and OpenAI.
    pub base_url: Url,
    pub api_key: String,
    pub model: String,
}

impl ChatConfig {
    /// True when the request goes to another machine. Only loopback addresses stay on this Mac.
    pub fn leaves_device(&self) -> bool {
        match self.base_url.host() {
            Some(Host::Domain(domain)) => domain != "localhost",
            Some(Host::Ipv4(address)) => !address.is_loopback(),
            Some(Host::Ipv6(address)) => !address.is_loopback(),
            None => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub jpeg_image: Option<Vec<u8>>,
}

#[derive(Serialize)]
pub(crate) struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<RequestMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct RequestMessage<'a> {
    role: ChatRole,
    content: Vec<ContentPart<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart<'a> {
    Text { text: &'a str },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

pub(crate) fn build_request_body<'a>(
    config: &'a ChatConfig,
    messages: &'a [ChatMessage],
) -> RequestBody<'a> {
    let messages = messages
        .iter()
        .map(|message| {
            let mut content = vec![ContentPart::Text {
                text: &message.text,
            }];
            if let Some(jpeg) = &message.jpeg_image {
                let encoded = base64::engine::general_purpose::STANDARD.encode(jpeg);
                content.push(ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: format!("data:image/jpeg;base64,{encoded}"),
                    },
                });
            }
            RequestMessage {
                role: message.role,
                content,
            }
        })
        .collect();
    RequestBody {
        model: &config.model,
        messages,
        stream: true,
    }
}
