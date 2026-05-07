use rag_common::{AppError, Config, Result};
use serde::Deserialize;
use serde_json::json;

const BATCH: usize = 8;

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedItem>,
}

#[derive(Debug, Deserialize)]
struct EmbedItem {
    index: usize,
    embedding: Vec<f32>,
}

pub async fn embed(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let cfg = Config::get();
    let client = reqwest::Client::new();
    let url = format!(
        "{}/embeddings",
        cfg.llamacpp_embed_url.trim_end_matches('/')
    );

    let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(BATCH) {
        let body = json!({
            "model": cfg.llamacpp_embed_model,
            "input": chunk,
        });
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()
            .map_err(AppError::http)?;
        let mut parsed: EmbedResponse = resp.json().await?;
        parsed.data.sort_by_key(|d| d.index);
        for item in parsed.data {
            if item.embedding.len() != cfg.embed_dim as usize {
                return Err(AppError::DimMismatch {
                    expected: cfg.embed_dim as usize,
                    got: item.embedding.len(),
                });
            }
            out.push(item.embedding);
        }
    }
    Ok(out)
}
