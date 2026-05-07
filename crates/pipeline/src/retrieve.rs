use rag_common::{Config, Result};
use rag_embed::embed_one;
use rag_search::{dense_search, get_qdrant_client, rerank};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RetrieveOpts {
    pub top_k: Option<u64>,
    pub top_n: Option<u64>,
    pub rerank: Option<bool>,
    pub filter: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievedDoc {
    pub id: String,
    pub text: String,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "rerankScore")]
    pub rerank_score: Option<f32>,
    pub source: String,
    #[serde(rename = "chunkId")]
    pub chunk_id: i64,
    pub headings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

pub async fn retrieve(query: &str, opts: RetrieveOpts) -> Result<Vec<RetrievedDoc>> {
    let cfg = Config::get();
    let top_k = opts.top_k.unwrap_or(cfg.top_k_retrieve);
    let top_n = opts.top_n.unwrap_or(cfg.top_k_rerank) as usize;

    let qvec = embed_one(query).await?;
    let client = get_qdrant_client();
    let hits = dense_search(&client, qvec, top_k, opts.filter.clone()).await?;

    let candidates: Vec<RetrievedDoc> = hits
        .into_iter()
        .map(|h| RetrievedDoc {
            id: h.id,
            text: h
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            score: h.score,
            rerank_score: None,
            source: h
                .payload
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            chunk_id: h
                .payload
                .get("chunk_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1),
            headings: h
                .payload
                .get("headings")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            kind: h
                .payload
                .get("kind")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
        .collect();

    if matches!(opts.rerank, Some(false)) {
        let truncated: Vec<RetrievedDoc> = candidates.into_iter().take(top_n).collect();
        return Ok(truncated);
    }

    let passages: Vec<String> = candidates.iter().map(|c| c.text.clone()).collect();
    let scored = rerank(query, &passages, top_n).await?;
    let out = scored
        .into_iter()
        .filter_map(|s| {
            candidates.get(s.index).map(|c| RetrievedDoc {
                rerank_score: Some(s.score),
                ..c.clone()
            })
        })
        .collect();
    Ok(out)
}
