//! Qdrant REST API クライアント（reqwest 直叩き、`@qdrant/js-client-rest` 互換）。

use rag_common::{AppError, Config, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DENSE: &str = "dense";

#[derive(Debug, Clone)]
pub struct QdrantClient {
    base: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl QdrantClient {
    pub fn new(base: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            base: base.into(),
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(k) => rb.header("api-key", k),
            None => rb,
        }
    }

    pub async fn raw_get(&self, path: &str) -> Result<Value> {
        let r = self.auth(self.client.get(self.url(path))).send().await?;
        let r = r.error_for_status().map_err(AppError::http)?;
        let v: Value = r.json().await?;
        Ok(v)
    }

    pub async fn raw_post(&self, path: &str, body: &Value) -> Result<Value> {
        let r = self
            .auth(self.client.post(self.url(path)).json(body))
            .send()
            .await?;
        let r = r.error_for_status().map_err(AppError::http)?;
        let v: Value = r.json().await?;
        Ok(v)
    }

    pub async fn raw_put(&self, path: &str, body: &Value) -> Result<Value> {
        let r = self
            .auth(self.client.put(self.url(path)).json(body))
            .send()
            .await?;
        let r = r.error_for_status().map_err(AppError::http)?;
        let v: Value = r.json().await?;
        Ok(v)
    }

    pub async fn raw_delete(&self, path: &str) -> Result<()> {
        let r = self.auth(self.client.delete(self.url(path))).send().await?;
        let _ = r.error_for_status().map_err(AppError::http)?;
        Ok(())
    }
}

pub fn get_qdrant_client() -> QdrantClient {
    let cfg = Config::get();
    QdrantClient::new(cfg.qdrant_url.clone(), cfg.qdrant_api_key.clone())
}

pub async fn ensure_collection(client: &QdrantClient, recreate: bool) -> Result<()> {
    let cfg = Config::get();
    let coll = &cfg.qdrant_collection;
    let path = format!("/collections/{coll}");

    if recreate {
        let _ = client.raw_delete(&path).await; // 既存なくても無視
    }

    // 存在確認
    let exists = client
        .raw_get(&format!("/collections/{coll}/exists"))
        .await
        .ok()
        .and_then(|v| v.get("result").cloned())
        .and_then(|v| v.get("exists").cloned())
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if exists && !recreate {
        return Ok(());
    }

    let body = json!({
        "vectors": {
            DENSE: {
                "size": cfg.embed_dim,
                "distance": "Cosine",
                "on_disk": false,
            }
        },
        "hnsw_config": { "m": 16, "ef_construct": 128 },
        "optimizers_config": { "indexing_threshold": 20000 },
    });
    client.raw_put(&path, &body).await?;

    // payload index
    for field in ["source", "kind", "url", "chunk_id"] {
        let schema = if field == "chunk_id" {
            "integer"
        } else {
            "keyword"
        };
        let body = json!({ "field_name": field, "field_schema": schema });
        let _ = client
            .raw_put(&format!("/collections/{coll}/index"), &body)
            .await;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointPayload {
    pub text: String,
    pub source: String,
    pub chunk_id: i64,
    pub headings: Vec<String>,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

pub async fn upsert_points(
    client: &QdrantClient,
    points: Vec<(String, Vec<f32>, PointPayload)>,
) -> Result<()> {
    let cfg = Config::get();
    let coll = &cfg.qdrant_collection;

    let pts: Vec<Value> = points
        .into_iter()
        .map(|(id, vec, payload)| {
            json!({
                "id": id,
                "vector": { DENSE: vec },
                "payload": payload,
            })
        })
        .collect();

    let body = json!({ "points": pts });
    client
        .raw_put(&format!("/collections/{coll}/points?wait=false"), &body)
        .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ScoredPoint {
    pub id: String,
    pub score: f32,
    pub payload: serde_json::Map<String, Value>,
}

pub async fn dense_search(
    client: &QdrantClient,
    vector: Vec<f32>,
    limit: u64,
    filter: Option<Value>,
) -> Result<Vec<ScoredPoint>> {
    let cfg = Config::get();
    let coll = &cfg.qdrant_collection;

    let mut body = json!({
        "query": vector,
        "using": DENSE,
        "limit": limit,
        "with_payload": true,
    });
    if let Some(f) = filter {
        body["filter"] = f;
    }
    let res = client
        .raw_post(&format!("/collections/{coll}/points/query"), &body)
        .await?;

    let arr = res
        .get("result")
        .and_then(|r| r.get("points"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(arr.len());
    for p in arr {
        let id = match p.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => continue,
        };
        let score = p.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let payload = p
            .get("payload")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        out.push(ScoredPoint { id, score, payload });
    }
    Ok(out)
}
