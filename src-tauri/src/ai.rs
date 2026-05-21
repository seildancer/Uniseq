use std::time::Duration;

const AI_HTTP_TIMEOUT_SECS: u64 = 90;
const GOOGLE_MODEL: &str = "gemini-2.5-flash-lite";
const GOOGLE_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

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
struct GoogleAiConfig {
    api_key: String,
    model: String,
    api_base: String,
}

pub fn chat_completion(
    system_prompt: &str,
    prior_messages: &[ChatMessage],
    latest_user_message: &str,
    api_key: &str,
) -> Result<String, ChatError> {
    let config = GoogleAiConfig::from_runtime(api_key)?;
    let mut contents = Vec::<serde_json::Value>::with_capacity(prior_messages.len() + 1);
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
        contents.push(serde_json::json!({
            "role": if role == "assistant" { "model" } else { "user" },
            "parts": [
                {
                    "text": content,
                }
            ],
        }));
    }

    contents.push(serde_json::json!({
        "role": "user",
        "parts": [
            {
                "text": latest_user_message,
            }
        ],
    }));

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(AI_HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|error| ChatError::Request(format!("failed to build HTTP client: {error}")))?;
    let response = client
        .post(format!(
            "{}/models/{}:generateContent",
            config.api_base, config.model
        ))
        .header("x-goog-api-key", config.api_key)
        .json(&serde_json::json!({
            "system_instruction": {
                "parts": [
                    {
                        "text": system_prompt,
                    }
                ]
            },
            "contents": contents,
        }))
        .send()
        .map_err(|error| ChatError::Request(format!("Gemini request failed: {error}")))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| ChatError::Request(format!("failed to read Gemini response: {error}")))?;
    if !status.is_success() {
        let detail = body.trim();
        return Err(ChatError::Request(format!(
            "Gemini returned {}{}",
            status,
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        )));
    }

    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| ChatError::Request(format!("failed to parse Gemini response JSON: {error}")))?;
    gemini_message_text(&value)
        .ok_or_else(|| ChatError::Request("Gemini response did not contain assistant text".to_owned()))
}

impl GoogleAiConfig {
    fn from_runtime(api_key: &str) -> Result<Self, ChatError> {
        let api_key = api_key.trim().to_owned();
        if api_key.is_empty() {
            return Err(ChatError::Config("Gemini API key is required".to_owned()));
        }
        let model = GOOGLE_MODEL.to_owned();
        let api_base = GOOGLE_API_BASE
            .trim()
            .trim_end_matches('/')
            .to_owned();
        Ok(Self {
            api_key,
            model,
            api_base,
        })
    }
}

fn gemini_message_text(response: &serde_json::Value) -> Option<String> {
    let parts = response
        .get("candidates")?
        .as_array()?
        .first()?
        .get("content")?
        .get("parts")?
        .as_array()?;
    let mut joined = String::new();
    for part in parts {
        if let Some(text) = part.get("text").and_then(serde_json::Value::as_str) {
            joined.push_str(text);
        }
    }
    (!joined.is_empty()).then_some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_message_text_joins_multiple_text_parts() {
        let value = serde_json::json!({
            "candidates": [
                {
                    "content": {
                        "parts": [
                            { "text": "hello" },
                            { "text": " world" }
                        ]
                    }
                }
            ]
        });

        assert_eq!(gemini_message_text(&value).as_deref(), Some("hello world"));
    }

    #[test]
    fn google_ai_config_requires_non_empty_api_key() {
        assert_eq!(
            GoogleAiConfig::from_runtime("   ").unwrap_err(),
            ChatError::Config("Gemini API key is required".to_owned())
        );
    }
}
