use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use rag_common::{AppError, Config, Result};
use rag_llm::{generate, generate_stream, RetrievedDocLite};
use rag_pipeline::{
    expand_path, ingest_path as pipeline_ingest_path, ingest_paths, retrieve, RetrieveOpts,
    RetrievedDoc,
};
use rag_search::{ensure_collection, get_qdrant_client};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/ingest", post(ingest_endpoint))
        .route("/ingest/upload", post(ingest_upload))
        .route("/search", post(search_endpoint))
        .route("/search/stream", post(search_stream))
        .route("/reindex", post(reindex))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(TimeoutLayer::with_status_code(
                    axum::http::StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(15 * 60),
                )),
        )
}

pub async fn run() -> Result<()> {
    let cfg = Config::get();
    let app = router();
    let addr = format!("{}:{}", cfg.rag_api_host, cfg.rag_api_port);
    tracing::info!(host = %cfg.rag_api_host, port = cfg.rag_api_port, "Starting Rust API");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn status() -> Result<Json<serde_json::Value>> {
    let cfg = Config::get();
    let http = reqwest::Client::new();

    let q_url = format!("{}/readyz", cfg.qdrant_url.trim_end_matches('/'));
    let o_url = format!("{}/api/tags", cfg.ollama_host.trim_end_matches('/'));
    let d_url = format!("{}/health", cfg.docling_url.trim_end_matches('/'));

    let (q, o, d) = tokio::join!(
        async {
            http.get(&q_url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        },
        async {
            http.get(&o_url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        },
        async {
            http.get(&d_url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        },
    );

    let client = get_qdrant_client();
    let collections = client
        .raw_get("/collections")
        .await
        .ok()
        .and_then(|v| v.get("result").cloned())
        .and_then(|v| v.get("collections").cloned())
        .unwrap_or(serde_json::Value::Array(vec![]));

    Ok(Json(json!({
        "qdrant": if q { "ok" } else { "down" },
        "ollama": if o { "ok" } else { "down" },
        "docling": if d { "ok" } else { "down" },
        "backend": cfg.rag_backend,
        "collections": collections,
    })))
}

#[derive(Debug, Deserialize)]
struct IngestRequest {
    paths: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    collection: Option<String>,
}

async fn ingest_endpoint(Json(req): Json<IngestRequest>) -> Result<Json<serde_json::Value>> {
    if req.paths.is_empty() {
        return Err(AppError::Validation("paths must not be empty".into()));
    }
    let mut expanded: Vec<String> = Vec::new();
    for p in &req.paths {
        expanded.extend(expand_path(p).await?);
    }
    let total = expanded.len() as u64;
    let stats = ingest_paths(expanded).await?;
    Ok(Json(json!({
        "ingested": stats.ingested,
        "chunks": stats.chunks,
        "errors": stats.errors,
        "total": total,
    })))
}

async fn ingest_upload(mut multipart: Multipart) -> Result<Json<serde_json::Value>> {
    let mut saved_path: Option<String> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("multipart: {e}")))?
    {
        if field.name() == Some("file") {
            let filename = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "upload.bin".to_string());
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::Validation(format!("multipart bytes: {e}")))?;
            let dir = std::path::Path::new("data").join("upload");
            tokio::fs::create_dir_all(&dir).await?;
            let p = dir.join(&filename);
            tokio::fs::write(&p, &bytes).await?;
            saved_path = Some(p.display().to_string());
            break;
        }
    }
    let saved = saved_path.ok_or_else(|| AppError::Validation("file field required".into()))?;
    let stat = pipeline_ingest_path(&saved).await?;
    Ok(Json(json!({ "path": saved, "chunks": stat.chunks })))
}

#[derive(Debug, Deserialize)]
struct SearchRequest {
    query: String,
    #[serde(default)]
    top_k: Option<u64>,
    #[serde(default)]
    top_n: Option<u64>,
    #[serde(default)]
    rerank: Option<bool>,
    #[serde(default = "default_true")]
    generate: bool,
}
fn default_true() -> bool {
    true
}

async fn search_endpoint(Json(req): Json<SearchRequest>) -> Result<Json<serde_json::Value>> {
    if req.query.is_empty() || req.query.len() > 2000 {
        return Err(AppError::Validation("query length out of range".into()));
    }
    let docs = retrieve(
        &req.query,
        RetrieveOpts {
            top_k: req.top_k,
            top_n: req.top_n,
            rerank: req.rerank,
            filter: None,
        },
    )
    .await?;

    let answer = if req.generate {
        let lite: Vec<RetrievedDocLite> = docs
            .iter()
            .map(|d| RetrievedDocLite {
                source: d.source.clone(),
                headings: d.headings.clone(),
                text: d.text.clone(),
            })
            .collect();
        Some(generate(&req.query, &lite).await?)
    } else {
        None
    };
    Ok(Json(json!({
        "answer": answer,
        "sources": docs,
    })))
}

async fn search_stream(Json(req): Json<SearchRequest>) -> Response {
    let docs_res = retrieve(
        &req.query,
        RetrieveOpts {
            top_k: req.top_k,
            top_n: req.top_n,
            rerank: req.rerank,
            filter: None,
        },
    )
    .await;

    let docs: Vec<RetrievedDoc> = match docs_res {
        Ok(d) => d,
        Err(e) => return e.into_response(),
    };

    let lite: Vec<RetrievedDocLite> = docs
        .iter()
        .map(|d| RetrievedDocLite {
            source: d.source.clone(),
            headings: d.headings.clone(),
            text: d.text.clone(),
        })
        .collect();
    let query = req.query.clone();
    let docs_clone = docs.clone();

    let stream = async_stream::stream! {
        // 1 行目: sources
        let header = serde_json::json!({ "type": "sources", "sources": docs_clone });
        let mut header_bytes = serde_json::to_vec(&header).unwrap_or_default();
        header_bytes.push(b'\n');
        yield Ok::<_, std::io::Error>(bytes::Bytes::from(header_bytes));

        // 2 行目: 区切り
        yield Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"---\n"));

        // 3 行目以降: LLM トークン
        let mut llm = generate_stream(&query, &lite);
        while let Some(item) = llm.next().await {
            match item {
                Ok(s) => yield Ok::<_, std::io::Error>(bytes::Bytes::from(s.into_bytes())),
                Err(e) => {
                    let err = format!("\n[ERROR] {e}");
                    yield Ok::<_, std::io::Error>(bytes::Bytes::from(err.into_bytes()));
                    return;
                }
            }
        }
    };

    let body = Body::from_stream(stream);
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; charset=utf-8")
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn reindex() -> Result<Json<serde_json::Value>> {
    let cfg = Config::get();
    let client = get_qdrant_client();
    ensure_collection(&client, true).await?;
    Ok(Json(json!({
        "collection": cfg.qdrant_collection,
        "recreated": true,
    })))
}
