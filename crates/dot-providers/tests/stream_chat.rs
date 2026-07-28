use dot_providers::{ChatConfig, ChatMessage, ChatRole, ProviderError, stream_chat};
use futures_util::StreamExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config_for(server: &MockServer) -> ChatConfig {
    ChatConfig {
        base_url: format!("{}/v1", server.uri()).parse().unwrap(),
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
    }
}

fn question(text: &str) -> Vec<ChatMessage> {
    vec![ChatMessage {
        role: ChatRole::User,
        text: text.to_string(),
        jpeg_image: None,
    }]
}

async fn server_answering(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn sse(events: &[&str]) -> ResponseTemplate {
    let body: String = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect();
    ResponseTemplate::new(200).set_body_raw(body, "text/event-stream")
}

async fn collect_answer(
    config: &ChatConfig,
    messages: &[ChatMessage],
) -> Vec<Result<String, ProviderError>> {
    let client = reqwest::Client::new();
    stream_chat(&client, config, messages).collect().await
}

async fn first_error(server: &MockServer) -> ProviderError {
    let items = collect_answer(&config_for(server), &question("hi")).await;
    items
        .into_iter()
        .find_map(Result::err)
        .expect("expected an error")
}

#[tokio::test]
async fn yields_text_deltas_in_order_and_skips_chunks_without_text() {
    let server = server_answering(sse(&[
        r#"{"choices":[{"delta":{"role":"assistant","content":null}}]}"#,
        r#"{"choices":[{"delta":{"content":"Par"}}]}"#,
        r#"{"choices":[{"delta":{"content":""}}]}"#,
        r#"{"choices":[{"delta":{"content":"is"}}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        r#"{"choices":[],"usage":{"completion_tokens":2}}"#,
        "[DONE]",
    ]))
    .await;

    let items = collect_answer(&config_for(&server), &question("capital of France?")).await;

    assert_eq!(items, vec![Ok("Par".to_string()), Ok("is".to_string())]);
}

#[tokio::test]
async fn posts_model_bearer_key_and_stream_flag_to_chat_completions_under_the_base_url() {
    let server = server_answering(sse(&["[DONE]"])).await;

    collect_answer(&config_for(&server), &question("hi")).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/v1/chat/completions");
    assert_eq!(requests[0].headers["authorization"], "Bearer test-key");
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "model": "test-model",
            "stream": true,
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}]
        })
    );
}

#[tokio::test]
async fn sends_an_image_as_a_jpeg_data_uri_after_the_text() {
    let server = server_answering(sse(&["[DONE]"])).await;
    let messages = vec![ChatMessage {
        role: ChatRole::User,
        text: "what is this?".to_string(),
        jpeg_image: Some(vec![0xFF, 0xD8, 0xFF]),
    }];

    collect_answer(&config_for(&server), &messages).await;

    let body: serde_json::Value = server.received_requests().await.unwrap()[0]
        .body_json()
        .unwrap();
    assert_eq!(
        body["messages"][0]["content"],
        serde_json::json!([
            {"type": "text", "text": "what is this?"},
            {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,/9j/"}}
        ])
    );
}

#[tokio::test]
async fn sends_system_and_assistant_roles_in_lowercase() {
    let server = server_answering(sse(&["[DONE]"])).await;
    let messages = vec![
        ChatMessage {
            role: ChatRole::System,
            text: "be brief".to_string(),
            jpeg_image: None,
        },
        ChatMessage {
            role: ChatRole::Assistant,
            text: "ok".to_string(),
            jpeg_image: None,
        },
    ];

    collect_answer(&config_for(&server), &messages).await;

    let body: serde_json::Value = server.received_requests().await.unwrap()[0]
        .body_json()
        .unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["role"], "assistant");
}

#[tokio::test]
async fn status_401_and_403_are_auth_failed() {
    for status in [401, 403] {
        let server = server_answering(ResponseTemplate::new(status)).await;
        assert_eq!(
            first_error(&server).await,
            ProviderError::AuthFailed,
            "status {status}"
        );
    }
}

#[tokio::test]
async fn gemini_bad_key_400_is_auth_failed() {
    let body = r#"[{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}]"#;
    let server = server_answering(ResponseTemplate::new(400).set_body_string(body)).await;

    assert_eq!(first_error(&server).await, ProviderError::AuthFailed);
}

#[tokio::test]
async fn other_400_keeps_status_and_body() {
    let server =
        server_answering(ResponseTemplate::new(400).set_body_string("context too long")).await;

    assert_eq!(
        first_error(&server).await,
        ProviderError::Other {
            status: Some(400),
            body: "context too long".to_string()
        }
    );
}

#[tokio::test]
async fn status_404_is_model_not_found() {
    let server = server_answering(ResponseTemplate::new(404)).await;

    assert_eq!(first_error(&server).await, ProviderError::ModelNotFound);
}

#[tokio::test]
async fn status_429_is_rate_limited() {
    let server = server_answering(ResponseTemplate::new(429)).await;

    assert_eq!(first_error(&server).await, ProviderError::RateLimited);
}

#[tokio::test]
async fn status_500_keeps_status_and_body() {
    let server = server_answering(ResponseTemplate::new(500).set_body_string("boom")).await;

    assert_eq!(
        first_error(&server).await,
        ProviderError::Other {
            status: Some(500),
            body: "boom".to_string()
        }
    );
}

#[tokio::test]
async fn closed_port_is_unreachable() {
    // wiremock pools its servers, so a dropped MockServer keeps listening; use a port nobody holds.
    let free_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let config = ChatConfig {
        base_url: format!("http://127.0.0.1:{free_port}/v1").parse().unwrap(),
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
    };

    let items = collect_answer(&config, &question("hi")).await;

    assert!(
        matches!(items.as_slice(), [Err(ProviderError::Unreachable(_))]),
        "got {items:?}"
    );
}

#[tokio::test]
async fn error_chunk_mid_stream_ends_the_answer_with_its_body() {
    let server = server_answering(sse(&[
        r#"{"choices":[{"delta":{"content":"Par"}}]}"#,
        r#"{"error":{"code":500,"message":"slot unavailable"}}"#,
    ]))
    .await;

    let items = collect_answer(&config_for(&server), &question("hi")).await;

    assert_eq!(items.len(), 2);
    assert_eq!(items[0], Ok("Par".to_string()));
    assert!(
        matches!(&items[1], Err(ProviderError::Other { status: None, body }) if body.contains("slot unavailable")),
        "got {:?}",
        items[1]
    );
}

#[tokio::test]
async fn stream_that_ends_without_done_is_unreachable_after_the_text_so_far() {
    let server = server_answering(sse(&[r#"{"choices":[{"delta":{"content":"Par"}}]}"#])).await;

    let items = collect_answer(&config_for(&server), &question("hi")).await;

    assert_eq!(items.len(), 2);
    assert_eq!(items[0], Ok("Par".to_string()));
    assert!(
        matches!(items[1], Err(ProviderError::Unreachable(_))),
        "got {:?}",
        items[1]
    );
}

#[tokio::test]
async fn unparsable_chunk_is_an_error_with_the_raw_data() {
    let server = server_answering(sse(&["not json", "[DONE]"])).await;

    assert_eq!(
        first_error(&server).await,
        ProviderError::Other {
            status: None,
            body: "not json".to_string()
        }
    );
}
