use rag_common::{AppError, Config, Result};
use serde::Deserialize;
use serde_json::json;

const BATCH: usize = 16;

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

pub async fn embed(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let cfg = Config::get();
    let client = reqwest::Client::new();
    let url = format!("{}/api/embed", cfg.ollama_host.trim_end_matches('/'));

    let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(BATCH) {
        let body = json!({
            "model": cfg.ollama_embed_model,
            "input": chunk,
            "truncate": true,
        });
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()
            .map_err(AppError::http)?;
        let parsed: EmbedResponse = resp.json().await?;
        for v in parsed.embeddings {
            if v.len() != cfg.embed_dim as usize {
                return Err(AppError::DimMismatch {
                    expected: cfg.embed_dim as usize,
                    got: v.len(),
                });
            }
            out.push(v);
        }
    }
    Ok(out)
}
