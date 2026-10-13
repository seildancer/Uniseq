use std::time::Duration;
use tokio::sync::watch;

const AI_HTTP_TIMEOUT_SECS: u64 = 90;
const OPENAI_MODEL: &str = "gpt-6-luna";
const OPENAI_API_BASE: &str = "https://api.openai.com/v1";
const OPENAI_ALLOWED_MODELS: &[&str] = &["gpt-6-luna", "gpt-6.1-sol"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatError {
    Config(String),
    Request(String),
}

pub struct StreamCompletion {
    pub text: String,
    pub stopped: bool,
}

pub async fn chat_completion_stream(
    system_prompt: &str,
    prior_messages: &[ChatMessage],
    latest_user_message: &str,
    api_key: &str,
    model: &str,
    cancelled: watch::Receiver<bool>,
    on_delta: impl FnMut(&str),
) -> Result<StreamCompletion, ChatError> {
    let config = OpenAiConfig::from_runtime(api_key, model)?;
    let mut body = response_request_body(
        system_prompt,
        prior_messages,
        latest_user_message,
        &config.model,
    )?;
    body["stream"] = serde_json::json!(true);
    request_stream(&config, &body, cancelled, on_delta).await
}

async fn request_stream(
    config: &OpenAiConfig,
    body: &serde_json::Value,
    mut cancelled: watch::Receiver<bool>,
    mut on_delta: impl FnMut(&str),
) -> Result<StreamCompletion, ChatError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(AI_HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|error| ChatError::Request(error.to_string()))?;
    // Accumulation survives cancellation; dropping the future closes the HTTP response.
    let mut text = String::new();
    let result = async {
        let mut response = client
            .post(format!("{}/responses", config.api_base))
            .bearer_auth(&config.api_key)
            .json(body)
            .send()
            .await
            .map_err(|error| ChatError::Request(format!("OpenAI request failed: {error}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            return Err(ChatError::Request(format!(
                "OpenAI returned {status}: {detail}"
            )));
        }
        let mut decoder = StreamDecoder::default();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| ChatError::Request(format!("Response interrupted: {error}")))?
        {
            for event in decoder.push(&chunk)? {
                match event {
                    StreamEvent::Delta(delta) => {
                        text.push_str(&delta);
                        on_delta(&delta);
                    }
                    StreamEvent::Complete => return Ok(()),
                }
            }
        }
        Err(ChatError::Request(
            "Response interrupted before completion. Please retry.".into(),
        ))
    };
    let stopped = tokio::select! {
        biased;
        _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => true,
        result = result => { result?; false },
    };
    if !stopped && text.is_empty() {
        return Err(ChatError::Request(
            "Response did not contain assistant text".into(),
        ));
    }
    Ok(StreamCompletion { text, stopped })
}

#[derive(Debug, PartialEq, Eq)]
enum StreamEvent {
    Delta(String),
    Complete,
}

#[derive(Default)]
struct StreamDecoder {
    pending: Vec<u8>,
    data: Vec<String>,
}

impl StreamDecoder {
    // Decode complete lines only, preserving UTF-8 split across network chunks.
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<StreamEvent>, ChatError> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line = self.pending.drain(..=end).collect::<Vec<_>>();
            let line = std::str::from_utf8(&line)
                .map_err(|error| ChatError::Request(error.to_string()))?
                .trim_end_matches(['\r', '\n']);
            if let Some(data) = line.strip_prefix("data:") {
                self.data
                    .push(data.strip_prefix(' ').unwrap_or(data).to_owned());
            } else if line.is_empty() && !self.data.is_empty() {
                let data = self.data.join("\n");
                self.data.clear();
                if data == "[DONE]" {
                    continue;
                }
                let value: serde_json::Value = serde_json::from_str(&data).map_err(|error| {
                    ChatError::Request(format!("Invalid stream event: {error}"))
                })?;
                match value["type"].as_str() {
                    Some("response.output_text.delta" | "response.refusal.delta") => {
                        if let Some(delta) = value["delta"].as_str() {
                            events.push(StreamEvent::Delta(delta.to_owned()));
                        }
                    }
                    Some("response.completed") => events.push(StreamEvent::Complete),
                    Some("error" | "response.failed" | "response.incomplete") => {
                        let detail = value["message"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value["response"].to_string());
                        return Err(ChatError::Request(format!("Response failed: {detail}")));
                    }
                    _ => {}
                }
            }
        }
        Ok(events)
    }
}

#[derive(Debug, Clone)]
struct OpenAiConfig {
    api_key: String,
    model: String,
    api_base: String,
}

pub fn chat_completion(
    system_prompt: &str,
    prior_messages: &[ChatMessage],
    latest_user_message: &str,
    api_key: &str,
    model: &str,
) -> Result<String, ChatError> {
    let config = OpenAiConfig::from_runtime(api_key, model)?;
    let body = response_request_body(
        system_prompt,
        prior_messages,
        latest_user_message,
        &config.model,
    )?;
    request_completion(&config, &body)
}

fn response_request_body(
    system_prompt: &str,
    prior_messages: &[ChatMessage],
    latest_user_message: &str,
    model: &str,
) -> Result<serde_json::Value, ChatError> {
    let mut input = Vec::<serde_json::Value>::with_capacity(prior_messages.len() + 1);
    for message in prior_messages {
        let role = message.role.trim();
        if role != "user" && role != "assistant" {
            return Err(ChatError::Request(format!(
                "unsupported AI chat role '{role}'"
            )));
        }
        let content = message.content.trim();
        if content.is_empty() {
            continue;
        }
        input.push(serde_json::json!({
            "role": role,
            "content": content,
        }));
    }

    input.push(serde_json::json!({
        "role": "user",
        "content": latest_user_message,
    }));
    Ok(serde_json::json!({
        "model": model,
        "instructions": system_prompt,
        "input": input,
        "reasoning": { "effort": "low" },
        "store": false,
    }))
}

fn request_completion(
    config: &OpenAiConfig,
    body: &serde_json::Value,
) -> Result<String, ChatError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(AI_HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|error| ChatError::Request(format!("failed to build HTTP client: {error}")))?;
    let response = client
        .post(format!("{}/responses", config.api_base))
        .bearer_auth(&config.api_key)
        .json(body)
        .send()
        .map_err(|error| ChatError::Request(format!("OpenAI request failed: {error}")))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| ChatError::Request(format!("failed to read OpenAI response: {error}")))?;
    if !status.is_success() {
        let detail = body.trim();
        return Err(ChatError::Request(format!(
            "OpenAI returned {}{}",
            status,
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        )));
    }

    let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| {
        ChatError::Request(format!("failed to parse OpenAI response JSON: {error}"))
    })?;
    if value.get("status").and_then(serde_json::Value::as_str) != Some("completed") {
        return Err(ChatError::Request(format!(
            "OpenAI response was not completed: {}",
            value
                .get("error")
                .filter(|value| !value.is_null())
                .or_else(|| value
                    .get("incomplete_details")
                    .filter(|value| !value.is_null()))
                .unwrap_or(&value["status"])
        )));
    }
    openai_message_text(&value).ok_or_else(|| {
        ChatError::Request("OpenAI response did not contain assistant text".to_owned())
    })
}

impl OpenAiConfig {
    fn from_runtime(api_key: &str, model: &str) -> Result<Self, ChatError> {
        let api_key = api_key.trim().to_owned();
        if api_key.is_empty() {
            return Err(ChatError::Config("OpenAI API key is required".to_owned()));
        }
        let model = normalize_model(model);
        let api_base = OPENAI_API_BASE.trim().trim_end_matches('/').to_owned();
        Ok(Self {
            api_key,
            model,
            api_base,
        })
    }
}

fn normalize_model(model: &str) -> String {
    let trimmed = model.trim();
    if OPENAI_ALLOWED_MODELS.contains(&trimmed) {
        trimmed.to_owned()
    } else {
        OPENAI_MODEL.to_owned()
    }
}

fn openai_message_text(response: &serde_json::Value) -> Option<String> {
    let output = response.get("output")?.as_array()?;
    let mut joined = String::new();
    for item in output {
        if item["type"] != "message" || item["role"] != "assistant" {
            continue;
        }
        for part in item
            .get("content")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = match part["type"].as_str() {
                Some("output_text") => part.get("text"),
                Some("refusal") => part.get("refusal"),
                _ => None,
            };
            if let Some(text) = text.and_then(serde_json::Value::as_str) {
                joined.push_str(text);
            }
        }
    }
    (!joined.is_empty()).then_some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn stream_decoder_preserves_fragmented_utf8_and_handles_crlf_refusals_and_comments() {
        let wire = concat!(
            ": keepalive\r\n\r\n",
            "event: response.output_text.delta\r\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"caf\u{e9}\"}\r\n\r\n",
            "data: {\"type\":\"response.refusal.delta\",\"delta\":\" refusal\"}\n\n",
            "data: {\"type\":\"response.completed\"}\n\n",
        );
        let mut decoder = StreamDecoder::default();
        let mut events = Vec::new();
        for byte in wire.as_bytes() {
            events.extend(decoder.push(&[*byte]).unwrap());
        }
        assert_eq!(
            events,
            vec![
                StreamEvent::Delta("caf\u{e9}".into()),
                StreamEvent::Delta(" refusal".into()),
                StreamEvent::Complete
            ]
        );
    }

    #[test]
    fn stream_decoder_rejects_generation_errors_and_invalid_data() {
        for event in [
            "data: {\"type\":\"error\",\"message\":\"quota exceeded\"}\n\n",
            "data: {\"type\":\"response.failed\",\"response\":{\"error\":\"failed\"}}\n\n",
            "data: {\"type\":\"response.incomplete\",\"response\":{\"incomplete_details\":{\"reason\":\"limit\"}}}\n\n",
            "data: invalid json\n\n",
        ] {
            assert!(StreamDecoder::default().push(event.as_bytes()).is_err());
        }
    }

    fn stream_server(
        wire: &'static str,
    ) -> (OpenAiConfig, std::thread::JoinHandle<serde_json::Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let (start, length) = loop {
                let n = socket.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    assert!(headers.starts_with("POST /v1/responses"));
                    assert!(
                        headers
                            .to_ascii_lowercase()
                            .contains("authorization: bearer test-key")
                    );
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    break (end + 4, length);
                }
            };
            while bytes.len() < start + length {
                let n = socket.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let body = serde_json::from_slice(&bytes[start..start + length]).unwrap();
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{wire}", wire.len()).unwrap();
            body
        });
        let mut config = OpenAiConfig::from_runtime("test-key", OPENAI_MODEL).unwrap();
        config.api_base = format!("http://{address}/v1");
        (config, server)
    }

    #[tokio::test]
    async fn streaming_http_delivers_text_and_rejects_a_disconnected_stream() {
        for (wire, success) in [
            (
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\ndata: {\"type\":\"response.completed\"}\n\n",
                true,
            ),
            (
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Partial\"}\n\n",
                false,
            ),
        ] {
            let (config, server) = stream_server(wire);
            let mut body =
                response_request_body("Scoped notes", &[], "Question", &config.model).unwrap();
            body["stream"] = serde_json::json!(true);
            let (_sender, cancelled) = watch::channel(false);
            let mut deltas = String::new();
            let result =
                request_stream(&config, &body, cancelled, |delta| deltas.push_str(delta)).await;
            let received = tokio::task::spawn_blocking(move || server.join().unwrap())
                .await
                .unwrap();
            assert_eq!(received, body);
            assert!(!deltas.is_empty());
            if success {
                let completion = result.unwrap();
                assert_eq!(completion.text, "Hello");
                assert!(!completion.stopped);
            } else {
                assert!(result.is_err());
                assert_eq!(deltas, "Partial");
            }
        }
    }

    #[tokio::test]
    async fn streaming_can_be_cancelled_before_network_activity() {
        let config = OpenAiConfig::from_runtime("test-key", OPENAI_MODEL).unwrap();
        let (sender, cancelled) = watch::channel(false);
        sender.send(true).unwrap();
        let result = request_stream(&config, &serde_json::json!({}), cancelled, |_| {
            panic!("No output expected")
        })
        .await
        .unwrap();
        assert!(result.stopped);
        assert!(result.text.is_empty());
    }

    #[tokio::test]
    async fn streaming_stop_preserves_partial_text_and_closes_idle_http_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buffer = [0; 4096];
            assert!(socket.read(&mut buffer).unwrap() > 0);
            let wire =
                "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Partial answer\"}\n\n";
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{wire}\r\n", wire.len()).unwrap();
            socket.flush().unwrap();
            // The client must disconnect without waiting for more bytes or a timeout.
            loop {
                if socket.read(&mut buffer).unwrap() == 0 {
                    break;
                }
            }
        });
        let mut config = OpenAiConfig::from_runtime("test-key", OPENAI_MODEL).unwrap();
        config.api_base = format!("http://{address}/v1");
        let (sender, cancelled) = watch::channel(false);
        let result = request_stream(
            &config,
            &serde_json::json!({ "stream": true }),
            cancelled,
            |_| {
                sender.send(true).unwrap();
            },
        )
        .await
        .unwrap();
        assert!(result.stopped);
        assert_eq!(result.text, "Partial answer");
        tokio::task::spawn_blocking(move || server.join().unwrap())
            .await
            .unwrap();
    }

    #[test]
    fn responses_request_preserves_context_and_history_without_remote_storage() {
        let history = vec![
            ChatMessage {
                role: "user".into(),
                content: "Earlier question".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "Earlier answer".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "  ".into(),
            },
        ];
        let body = response_request_body("Notes and memory", &history, "Follow-up", "gpt-6.1-sol")
            .unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "model": "gpt-6.1-sol",
                "instructions": "Notes and memory",
                "input": [
                    { "role": "user", "content": "Earlier question" },
                    { "role": "assistant", "content": "Earlier answer" },
                    { "role": "user", "content": "Follow-up" },
                ],
                "reasoning": { "effort": "low" },
                "store": false,
            })
        );
        assert!(
            response_request_body(
                "",
                &[ChatMessage {
                    role: "system".into(),
                    content: "Untrusted instructions".into(),
                }],
                "Question",
                OPENAI_MODEL
            )
            .is_err()
        );
    }

    // Exercise the actual HTTP path without an API key or paid API calls.
    #[test]
    fn responses_http_uses_bearer_auth_and_handles_success_and_failures() {
        let cases = [
            (
                "200 OK",
                r#"{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Answer"}]}]}"#,
                Ok("Answer"),
            ),
            (
                "200 OK",
                r#"{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"refusal","refusal":"Cannot help"}]}]}"#,
                Ok("Cannot help"),
            ),
            (
                "401 Unauthorized",
                r#"{"error":{"message":"Invalid API key"}}"#,
                Err("401 Unauthorized"),
            ),
            (
                "429 Too Many Requests",
                r#"{"error":{"message":"Quota exceeded"}}"#,
                Err("Quota exceeded"),
            ),
            (
                "200 OK",
                r#"{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"output":[]}"#,
                Err("max_output_tokens"),
            ),
            (
                "200 OK",
                r#"{"status":"failed","error":{"message":"Generation failed"}}"#,
                Err("Generation failed"),
            ),
            (
                "200 OK",
                r#"{"status":"completed","output":[]}"#,
                Err("did not contain assistant text"),
            ),
            (
                "200 OK",
                "invalid JSON",
                Err("failed to parse OpenAI response JSON"),
            ),
        ];
        for (status, response_body, expected) in cases {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                let (headers, body_start, content_length) = loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        break (headers, end + 4, length);
                    }
                };
                while bytes.len() < body_start + content_length {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                }
                let body: serde_json::Value =
                    serde_json::from_slice(&bytes[body_start..body_start + content_length])
                        .unwrap();
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}", response_body.len()).unwrap();
                (headers, body)
            });
            let mut config = OpenAiConfig::from_runtime(" test-key ", OPENAI_MODEL).unwrap();
            config.api_base = format!("http://{address}/v1");
            let body = response_request_body("Notes", &[], "Question", &config.model).unwrap();
            let result = request_completion(&config, &body);
            let (headers, received_body) = server.join().unwrap();
            assert!(headers.starts_with("POST /v1/responses HTTP/1.1\r\n"));
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-key")
            );
            assert_eq!(received_body, body);
            match expected {
                Ok(text) => assert_eq!(result.unwrap(), text),
                Err(detail) => assert!(
                    matches!(result, Err(ChatError::Request(message)) if message.contains(detail))
                ),
            }
        }
    }

    #[test]
    fn openai_message_text_joins_assistant_text_and_skips_reasoning() {
        let value = serde_json::json!({
            "output": [
                { "type": "reasoning", "summary": [{ "text": "hidden" }] },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        { "type": "output_text", "text": "hello" },
                        { "type": "output_text", "text": " world" }
                    ]
                }
            ]
        });

        assert_eq!(openai_message_text(&value).as_deref(), Some("hello world"));
    }

    #[test]
    fn openai_config_requires_non_empty_api_key() {
        assert_eq!(
            OpenAiConfig::from_runtime("   ", OPENAI_MODEL).unwrap_err(),
            ChatError::Config("OpenAI API key is required".to_owned())
        );
    }

    #[test]
    fn openai_config_uses_selected_model_when_allowed() {
        let config = OpenAiConfig::from_runtime("test-key", "gpt-6.1-sol").unwrap();
        assert_eq!(config.model, "gpt-6.1-sol");
    }

    #[test]
    fn openai_config_falls_back_for_unknown_model() {
        for model in ["not-a-model", "gemini-2.5-pro", ""] {
            let config = OpenAiConfig::from_runtime("test-key", model).unwrap();
            assert_eq!(config.model, OPENAI_MODEL);
        }
    }
}
