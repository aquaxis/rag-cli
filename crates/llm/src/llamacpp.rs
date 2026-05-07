use crate::LlmStream;
use futures_util::stream::StreamExt;
use rag_common::{AppError, Config, Result};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
}

pub async fn generate(system: &str, user: &str) -> Result<String> {
    let cfg = Config::get();
    let url = format!(
        "{}/chat/completions",
        cfg.llamacpp_llm_url.trim_end_matches('/')
    );
    let body = json!({
        "model": cfg.llamacpp_llm_model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "stream": false,
        "temperature": 0.1,
        "top_p": 0.9,
        "max_tokens": 1024,
    });

    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .map_err(AppError::http)?;
    let parsed: ChatCompletionResponse = resp.json().await?;
    Ok(parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message)
        .map(|m| m.content)
        .unwrap_or_default())
}

pub fn generate_stream(system: String, user: String) -> LlmStream {
    let s = async_stream::stream! {
        let cfg = Config::get();
        let url = format!(
            "{}/chat/completions",
            cfg.llamacpp_llm_url.trim_end_matches('/')
        );
        let body = json!({
            "model": cfg.llamacpp_llm_model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "stream": true,
            "temperature": 0.1,
            "top_p": 0.9,
            "max_tokens": 1024,
        });

        let resp = match reqwest::Client::new().post(&url).json(&body).send().await {
            Ok(r) => match r.error_for_status() {
                Ok(r) => r,
                Err(e) => { yield Err(AppError::http(e)); return; }
            },
            Err(e) => { yield Err(AppError::http(e)); return; }
        };

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Err(e) => { yield Err(AppError::http(e)); return; }
                Ok(bytes) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(idx) = buf.find('\n') {
                        let line: String = buf.drain(..=idx).collect();
                        let line = line.trim();
                        if !line.starts_with("data:") { continue; }
                        let data = line.trim_start_matches("data:").trim();
                        if data == "[DONE]" { return; }
                        match serde_json::from_str::<serde_json::Value>(data) {
                            Ok(v) => {
                                if let Some(content) = v.get("choices")
                                    .and_then(|c| c.as_array())
                                    .and_then(|a| a.first())
                                    .and_then(|c| c.get("delta"))
                                    .and_then(|d| d.get("content"))
                                    .and_then(|s| s.as_str())
                                {
                                    if !content.is_empty() {
                                        yield Ok(content.to_string());
                                    }
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                }
            }
        }
    };
    Box::pin(s)
}
