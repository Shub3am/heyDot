# dot-providers

Owns: sending one chat request to an OpenAI-compatible `/chat/completions` endpoint and streaming the answer's text back, with the request's image as a JPEG data URI, and turning HTTP and stream failures into `ProviderError`. Also says whether a base URL leaves this Mac (`ChatConfig::leaves_device`).

Must not know about: llama-server, which provider or model sits behind the URL, conversation history, system prompts, settings or the UI. It does not start servers or retry.

Entry points: `stream_chat(&client, &config, &messages)`, a stream of text deltas; `ChatConfig::leaves_device()`.

Invariants and gotchas:
- `base_url` is the OpenAI-compatible root ending in `/v1`; the crate appends `/chat/completions`.
- Every request is `stream: true`. Chunks without text (role-only first chunk, finish chunk, usage-only chunk) yield nothing.
- An answer is complete only when `[DONE]` arrived. A stream that ends without it yields its text so far, then `Unreachable`, so a killed server never looks like a short answer.
- A chunk carrying `error` ends the stream with `Other` holding that error's JSON. A chunk that does not parse ends it with `Other` holding the raw data.
- Gemini's OpenAI endpoint answers a bad key with 400 "API key not valid" instead of 401; that maps to `AuthFailed` like 401 and 403.
- `leaves_device` is false only for `localhost`, `127.0.0.0/8` and `::1`. A LAN address such as `192.168.1.20` leaves the Mac.
- The caller owns the `reqwest::Client` and its timeouts. With no read timeout a stalled server stalls the stream forever.
- Tests use wiremock, which pools its servers: a dropped `MockServer` keeps listening and may answer the next test. "Unreachable" tests use a port freed from a `TcpListener`.

Called by: `app/src-tauri` (the `ask_text` command) and `crates/dot-runtime`'s ignored real-model test.
