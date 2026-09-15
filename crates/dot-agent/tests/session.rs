use std::time::Duration;

use dot_agent::{AgentEvent, PastTurn, Session, UserInput};
use dot_providers::{ChatConfig, ProviderError};
use futures_util::StreamExt;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config_for(base_url: String) -> ChatConfig {
    ChatConfig {
        base_url: format!("{base_url}/v1").parse().unwrap(),
        api_key: "test-key".to_owned(),
        model: "test-model".to_owned(),
    }
}

fn question(text: &str) -> UserInput {
    UserInput {
        text: text.to_owned(),
        jpeg_screenshot: None,
    }
}

fn question_with_screenshot(text: &str) -> UserInput {
    UserInput {
        text: text.to_owned(),
        jpeg_screenshot: Some(vec![0xFF, 0xD8, 0xFF]),
    }
}

/// A server that answers every request with `deltas` and `[DONE]`.
async fn server_answering(deltas: &[&str]) -> MockServer {
    let body: String = deltas
        .iter()
        .map(|delta| {
            format!(
                "data: {}\n\n",
                json!({"choices": [{"delta": {"content": delta}}]})
            )
        })
        .chain(["data: [DONE]\n\n".to_owned()])
        .collect();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;
    server
}

async fn server_failing_with(status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(status))
        .mount(&server)
        .await;
    server
}

/// A server that answers "Par" and then never finishes, like a model still thinking.
async fn stalled_server() -> ChatConfig {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let event = "data: {\"choices\":[{\"delta\":{\"content\":\"Par\"}}]}\n\n";
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n{:x}\r\n{event}\r\n",
                    event.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                // Reading until the client hangs up keeps the answer unfinished.
                let mut request = vec![0; 64 * 1024];
                while socket.read(&mut request).await.unwrap_or(0) > 0 {}
            });
        }
    });
    config_for(base_url)
}

async fn answer_events(
    session: &Session,
    config: &ChatConfig,
    input: UserInput,
) -> Vec<Result<AgentEvent, ProviderError>> {
    let http = reqwest::Client::new();
    session
        .ask(session.begin_answer(), &http, config, input)
        .collect()
        .await
}

async fn last_request_messages(server: &MockServer) -> Vec<serde_json::Value> {
    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = requests.last().unwrap().body_json().unwrap();
    body["messages"].as_array().unwrap().clone()
}

#[tokio::test]
async fn streams_thinking_then_the_answer_text() {
    let server = server_answering(&["Par", "is"]).await;

    let events = answer_events(
        &Session::default(),
        &config_for(server.uri()),
        question("Capital of France?"),
    )
    .await;

    assert_eq!(
        events,
        [
            Ok(AgentEvent::Thinking {
                forgot_earlier_turns: false
            }),
            Ok(AgentEvent::Delta("Par".to_owned())),
            Ok(AgentEvent::Delta("is".to_owned())),
        ]
    );
}

#[tokio::test]
async fn sends_the_system_prompt_then_the_question_with_its_screenshot() {
    let server = server_answering(&["Paris"]).await;

    answer_events(
        &Session::default(),
        &config_for(server.uri()),
        question_with_screenshot("What city is this?"),
    )
    .await;

    let messages = last_request_messages(&server).await;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(
        messages[1]["content"],
        json!([
            {"type": "text", "text": "What city is this?"},
            {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,/9j/"}}
        ])
    );
}

#[tokio::test]
async fn a_follow_up_carries_the_earlier_turn_as_text_without_its_screenshot() {
    let server = server_answering(&["Paris"]).await;
    let config = config_for(server.uri());
    let session = Session::default();
    answer_events(
        &session,
        &config,
        question_with_screenshot("What city is this?"),
    )
    .await;

    answer_events(&session, &config, question("How many people live there?")).await;

    let messages = last_request_messages(&server).await;
    assert_eq!(messages.len(), 4);
    assert_eq!(
        messages[1]["content"],
        json!([{"type": "text", "text": "What city is this?\n[screenshot from earlier turn]"}])
    );
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(
        messages[2]["content"],
        json!([{"type": "text", "text": "Paris"}])
    );
    assert_eq!(
        messages[3]["content"],
        json!([{"type": "text", "text": "How many people live there?"}])
    );
}

#[tokio::test]
async fn a_failed_answer_is_left_out_of_the_next_question() {
    let failing = server_failing_with(500).await;
    let answering = server_answering(&["Paris"]).await;
    let session = Session::default();

    let failed = answer_events(
        &session,
        &config_for(failing.uri()),
        question("What city is this?"),
    )
    .await;
    answer_events(
        &session,
        &config_for(answering.uri()),
        question("Try again?"),
    )
    .await;

    assert!(matches!(
        failed.last(),
        Some(Err(ProviderError::Other {
            status: Some(500),
            ..
        }))
    ));
    assert_eq!(last_request_messages(&answering).await.len(), 2);
}

#[tokio::test]
async fn new_chat_forgets_earlier_turns() {
    let server = server_answering(&["Paris"]).await;
    let config = config_for(server.uri());
    let session = Session::default();
    answer_events(&session, &config, question("What city is this?")).await;

    session.new_chat().await;
    answer_events(&session, &config, question("Hi")).await;

    assert_eq!(last_request_messages(&server).await.len(), 2);
}

#[tokio::test]
async fn a_new_question_stops_the_running_answer_and_keeps_its_text_so_far() {
    let stalled = stalled_server().await;
    let answering = server_answering(&["Madrid"]).await;
    let answering_config = config_for(answering.uri());
    let session = Session::default();
    let http = reqwest::Client::new();
    let mut first = std::pin::pin!(session.ask(
        session.begin_answer(),
        &http,
        &stalled,
        question("What is the capital of France?")
    ));
    assert_eq!(
        first.next().await,
        Some(Ok(AgentEvent::Thinking {
            forgot_earlier_turns: false
        }))
    );
    assert_eq!(
        first.next().await,
        Some(Ok(AgentEvent::Delta("Par".to_owned())))
    );

    let second = session.ask(
        session.begin_answer(),
        &http,
        &answering_config,
        question("And of Spain?"),
    );
    let rest_of_first: Vec<_> = tokio::time::timeout(Duration::from_secs(5), first.collect())
        .await
        .expect("the running answer did not stop");
    let second_events: Vec<_> = second.collect().await;

    assert!(rest_of_first.is_empty());
    assert_eq!(
        second_events.last(),
        Some(&Ok(AgentEvent::Delta("Madrid".to_owned())))
    );
    let messages = last_request_messages(&answering).await;
    assert_eq!(messages.len(), 4);
    assert_eq!(
        messages[2]["content"],
        json!([{"type": "text", "text": "Par"}])
    );
}

#[tokio::test]
async fn new_chat_stops_the_running_answer() {
    let stalled = stalled_server().await;
    let session = Session::default();
    let http = reqwest::Client::new();
    let mut first = std::pin::pin!(session.ask(
        session.begin_answer(),
        &http,
        &stalled,
        question("What is the capital of France?")
    ));
    first.next().await;
    first.next().await;

    let (rest_of_first, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(first.collect::<Vec<_>>(), session.new_chat())
    })
    .await
    .expect("New chat did not stop the running answer");

    assert!(rest_of_first.is_empty());
}

#[tokio::test]
async fn stop_answer_ends_the_running_answer_and_keeps_its_text_for_the_next_question() {
    let stalled = stalled_server().await;
    let answering = server_answering(&["Madrid"]).await;
    let session = Session::default();
    let http = reqwest::Client::new();
    let mut first = std::pin::pin!(session.ask(
        session.begin_answer(),
        &http,
        &stalled,
        question("What is the capital of France?")
    ));
    first.next().await;
    first.next().await;

    session.stop_answer();
    let rest_of_first: Vec<_> = tokio::time::timeout(Duration::from_secs(5), first.collect())
        .await
        .expect("Stop did not end the running answer");
    answer_events(
        &session,
        &config_for(answering.uri()),
        question("And of Spain?"),
    )
    .await;

    assert!(rest_of_first.is_empty());
    let messages = last_request_messages(&answering).await;
    assert_eq!(messages.len(), 4);
    assert_eq!(
        messages[2]["content"],
        json!([{"type": "text", "text": "Par"}])
    );
}

#[tokio::test]
async fn stop_before_the_question_is_sent_ends_it_without_asking_the_model() {
    let server = server_answering(&["Paris"]).await;
    let session = Session::default();
    let http = reqwest::Client::new();

    let answer = session.begin_answer();
    session.stop_answer();
    let events: Vec<_> = session
        .ask(
            answer,
            &http,
            &config_for(server.uri()),
            question("Capital of France?"),
        )
        .collect()
        .await;

    assert!(events.is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn new_chat_before_the_question_is_sent_keeps_it_out_of_the_new_chat() {
    let first_server = server_answering(&["Paris"]).await;
    let second_server = server_answering(&["Madrid"]).await;
    let session = Session::default();
    let http = reqwest::Client::new();

    let answer = session.begin_answer();
    session.new_chat().await;
    let events: Vec<_> = session
        .ask(
            answer,
            &http,
            &config_for(first_server.uri()),
            question("Capital of France?"),
        )
        .collect()
        .await;
    answer_events(
        &session,
        &config_for(second_server.uri()),
        question("And of Spain?"),
    )
    .await;

    assert!(events.is_empty());
    assert_eq!(last_request_messages(&second_server).await.len(), 2);
}

fn past_turn(question: &str, had_screenshot: bool, answer: &str) -> PastTurn {
    PastTurn {
        question: question.to_owned(),
        had_screenshot,
        answer: answer.to_owned(),
    }
}

#[tokio::test]
async fn resume_replaces_the_conversation_with_the_saved_turns() {
    let server = server_answering(&["Paris"]).await;
    let config = config_for(server.uri());
    let session = Session::default();
    answer_events(&session, &config, question("Forget this")).await;

    session
        .resume(vec![past_turn("What is this chart?", true, "Revenue.")])
        .await;
    answer_events(&session, &config, question("And the red line?")).await;

    let messages = last_request_messages(&server).await;
    assert_eq!(messages.len(), 4);
    assert_eq!(
        messages[1]["content"],
        json!([{"type": "text", "text": "What is this chart?\n[screenshot from earlier turn]"}])
    );
    assert_eq!(
        messages[2]["content"],
        json!([{"type": "text", "text": "Revenue."}])
    );
}

#[tokio::test]
async fn resume_keeps_only_the_last_nine_saved_turns() {
    let server = server_answering(&["ok"]).await;
    let config = config_for(server.uri());
    let session = Session::default();
    let saved_turns = (1..=12)
        .map(|number| past_turn(&format!("question {number}"), false, "answer"))
        .collect();

    session.resume(saved_turns).await;
    answer_events(&session, &config, question("question 13")).await;

    let messages = last_request_messages(&server).await;
    assert_eq!(messages.len(), 1 + 9 * 2 + 1);
    assert_eq!(
        messages[1]["content"],
        json!([{"type": "text", "text": "question 4"}])
    );
}

#[tokio::test]
async fn resume_stops_the_running_answer() {
    let stalled = stalled_server().await;
    let session = Session::default();
    let http = reqwest::Client::new();
    let mut first = std::pin::pin!(session.ask(
        session.begin_answer(),
        &http,
        &stalled,
        question("What is the capital of France?")
    ));
    first.next().await;
    first.next().await;

    let (rest_of_first, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(first.collect::<Vec<_>>(), session.resume(Vec::new()))
    })
    .await
    .expect("Opening a saved chat did not stop the running answer");

    assert!(rest_of_first.is_empty());
}
