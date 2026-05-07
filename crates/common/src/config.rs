use figment::providers::{Env, Serialized};
use figment::Figment;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_qdrant_url")]
    pub qdrant_url: String,
    #[serde(default)]
    pub qdrant_api_key: Option<String>,
    #[serde(default = "default_qdrant_collection")]
    pub qdrant_collection: String,
    #[serde(default = "default_rag_backend")]
    pub rag_backend: String,
    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,
    #[serde(default = "default_ollama_llm_model")]
    pub ollama_llm_model: String,
    #[serde(default = "default_ollama_embed_model")]
    pub ollama_embed_model: String,
    #[serde(default = "default_llamacpp_embed_url")]
    pub llamacpp_embed_url: String,
    #[serde(default = "default_llamacpp_llm_url")]
    pub llamacpp_llm_url: String,
    #[serde(default = "default_llamacpp_embed_model")]
    pub llamacpp_embed_model: String,
    #[serde(default = "default_llamacpp_llm_model")]
    pub llamacpp_llm_model: String,
    #[serde(default = "default_docling_url")]
    pub docling_url: String,
    #[serde(default = "default_reranker_model")]
    pub reranker_model: String,
    #[serde(default = "default_rag_api_host")]
    pub rag_api_host: String,
    #[serde(default = "default_rag_api_port")]
    pub rag_api_port: u16,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,
    #[serde(default = "default_top_k_retrieve")]
    pub top_k_retrieve: u64,
    #[serde(default = "default_top_k_rerank")]
    pub top_k_rerank: u64,
    #[serde(default = "default_embed_dim")]
    pub embed_dim: u64,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub rag_hf_cache_dir: Option<String>,
    #[serde(default)]
    pub rag_reranker_model_dir: Option<String>,
    #[serde(default = "default_rag_rerank_batch")]
    pub rag_rerank_batch: usize,
}

fn default_qdrant_url() -> String {
    "http://127.0.0.1:6333".to_string()
}
fn default_qdrant_collection() -> String {
    "rag_documents".to_string()
}
fn default_rag_backend() -> String {
    "ollama".to_string()
}
fn default_ollama_host() -> String {
    "http://127.0.0.1:11434".to_string()
}
fn default_ollama_llm_model() -> String {
    "qwen2.5:7b-instruct".to_string()
}
fn default_ollama_embed_model() -> String {
    "bge-m3".to_string()
}
fn default_llamacpp_embed_url() -> String {
    "http://127.0.0.1:8080/v1".to_string()
}
fn default_llamacpp_llm_url() -> String {
    "http://127.0.0.1:8081/v1".to_string()
}
fn default_llamacpp_embed_model() -> String {
    "bge-m3".to_string()
}
fn default_llamacpp_llm_model() -> String {
    "qwen2.5-7b-instruct".to_string()
}
fn default_docling_url() -> String {
    "http://127.0.0.1:5001".to_string()
}
fn default_reranker_model() -> String {
    "onnx-community/bge-reranker-v2-m3-ONNX".to_string()
}
fn default_rag_api_host() -> String {
    "127.0.0.1".to_string()
}
fn default_rag_api_port() -> u16 {
    7777
}
fn default_chunk_size() -> usize {
    512
}
fn default_chunk_overlap() -> usize {
    64
}
fn default_top_k_retrieve() -> u64 {
    20
}
fn default_top_k_rerank() -> u64 {
    5
}
fn default_embed_dim() -> u64 {
    1024
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_rag_rerank_batch() -> usize {
    8
}

impl Default for Config {
    fn default() -> Self {
        figment::Figment::from(figment::providers::Serialized::defaults(serde_json::json!(
            {}
        )))
        .extract::<Config>()
        .unwrap_or_else(|_| Self {
            qdrant_url: default_qdrant_url(),
            qdrant_api_key: None,
            qdrant_collection: default_qdrant_collection(),
            rag_backend: default_rag_backend(),
            ollama_host: default_ollama_host(),
            ollama_llm_model: default_ollama_llm_model(),
            ollama_embed_model: default_ollama_embed_model(),
            llamacpp_embed_url: default_llamacpp_embed_url(),
            llamacpp_llm_url: default_llamacpp_llm_url(),
            llamacpp_embed_model: default_llamacpp_embed_model(),
            llamacpp_llm_model: default_llamacpp_llm_model(),
            docling_url: default_docling_url(),
            reranker_model: default_reranker_model(),
            rag_api_host: default_rag_api_host(),
            rag_api_port: default_rag_api_port(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            top_k_retrieve: default_top_k_retrieve(),
            top_k_rerank: default_top_k_rerank(),
            embed_dim: default_embed_dim(),
            log_level: default_log_level(),
            rag_hf_cache_dir: None,
            rag_reranker_model_dir: None,
            rag_rerank_batch: default_rag_rerank_batch(),
        })
    }
}

static CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    pub fn load() -> &'static Config {
        CONFIG.get_or_init(|| {
            let _ = dotenvy::dotenv();
            Figment::from(Serialized::defaults(Config::default()))
                .merge(Env::raw())
                .extract()
                .expect("failed to load config")
        })
    }

    pub fn get() -> &'static Config {
        CONFIG.get().unwrap_or_else(Self::load)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_applied() {
        let c = Config::default();
        assert_eq!(c.qdrant_collection, "rag_documents");
        assert_eq!(c.rag_api_port, 7777);
        assert_eq!(c.embed_dim, 1024);
    }
}
