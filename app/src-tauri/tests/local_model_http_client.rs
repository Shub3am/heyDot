// Its own test binary because it sets HTTP_PROXY for the whole process.

use std::io::{Read, Write};
use std::net::TcpListener;

#[tokio::test]
async fn reaches_a_loopback_server_when_an_unreachable_http_proxy_is_configured() {
    // SAFETY: this binary has one test and it sets the variables before any other thread starts.
    unsafe {
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:1");
        std::env::set_var("http_proxy", "http://127.0.0.1:1");
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        let _ = connection.read(&mut request).unwrap();
        connection
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });

    let client = hey_dot_lib::local_model_http_client().unwrap();
    let response = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .expect("the request went to the proxy instead of the loopback server");

    assert_eq!(response.status(), 200);
}
