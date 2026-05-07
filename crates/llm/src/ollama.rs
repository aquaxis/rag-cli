use crate::LlmStream;
use futures_util::stream::StreamExt;
use rag_common::{AppError, Config, Result};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct ChatResponse {
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
    let url = format!("{}/api/chat", cfg.ollama_host.trim_end_matches('/'));
    let body = json!({
        "model": cfg.ollama_llm_model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "stream": false,
        "options": { "temperature": 0.1, "top_p": 0.9, "num_predict": 1024 },
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .map_err(AppError::http)?;
    let parsed: ChatResponse = resp.json().await?;
    Ok(parsed.message.map(|m| m.content).unwrap_or_default())
}

pub fn generate_stream(system: String, user: String) -> LlmStream {
    let s = async_stream::stream! {
        let cfg = Config::get();
        let url = format!("{}/api/chat", cfg.ollama_host.trim_end_matches('/'));
        let body = json!({
            "model": cfg.ollama_llm_model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "stream": true,
            "options": { "temperature": 0.1, "top_p": 0.9, "num_predict": 1024 },
        });

        let client = reqwest::Client::new();
        let resp = match client.post(&url).json(&body).send().await {
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
                        if line.is_empty() { continue; }
                        match serde_json::from_str::<serde_json::Value>(line) {
                            Ok(v) => {
                                if let Some(content) = v.get("message")
                                    .and_then(|m| m.get("content"))
                                    .and_then(|c| c.as_str())
                                {
                                    if !content.is_empty() {
                                        yield Ok(content.to_string());
                                    }
                                }
                                if v.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
                                    return;
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
