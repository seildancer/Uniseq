use std::time::Duration;

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
