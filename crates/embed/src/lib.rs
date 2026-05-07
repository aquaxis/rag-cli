pub mod llamacpp;
pub mod ollama;

use rag_common::{AppError, Config, Result};

pub async fn embed(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let cfg = Config::get();
    match cfg.rag_backend.as_str() {
        "ollama" => ollama::embed(texts).await,
        "llamacpp" => llamacpp::embed(texts).await,
        other => Err(AppError::Config(format!("unknown RAG_BACKEND: {other}"))),
    }
}

pub async fn embed_one(text: &str) -> Result<Vec<f32>> {
    let v = embed(&[text.to_string()]).await?;
    v.into_iter()
        .next()
        .ok_or_else(|| AppError::internal("empty embedding response"))
}
