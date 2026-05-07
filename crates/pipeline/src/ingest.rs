use futures_util::stream::{FuturesUnordered, StreamExt};
use rag_chunk::chunk_japanese;
use rag_common::{AppError, Result};
use rag_embed::embed;
use rag_ingest::{convert_any, is_url, web::read_url_list, ConvertedDoc, DocMetadata};
use rag_search::{ensure_collection, get_qdrant_client, upsert_points, PointPayload};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize)]
pub struct IngestStat {
    pub chunks: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IngestStats {
    pub ingested: u64,
    pub chunks: u64,
    pub errors: u64,
}

const BATCH: usize = 32;

pub async fn ingest_path(input: &str) -> Result<IngestStat> {
    let doc: ConvertedDoc = convert_any(input).await?;
    let frontmatter = match &doc.metadata {
        DocMetadata::File { frontmatter, .. } => frontmatter.clone(),
        DocMetadata::Web { .. } => None,
    };
    let chunks = chunk_japanese(&doc.source, &doc.markdown, frontmatter);
    if chunks.is_empty() {
        return Ok(IngestStat { chunks: 0 });
    }

    let client = get_qdrant_client();
    ensure_collection(&client, false).await?;

    let kind = doc.kind_str().to_string();
    let url = doc.url().map(|s| s.to_string());

    let mut total: u64 = 0;
    for slice in chunks.chunks(BATCH) {
        let texts: Vec<String> = slice.iter().map(|c| c.text.clone()).collect();
        let vectors = embed(&texts).await?;
        let mut points: Vec<(String, Vec<f32>, PointPayload)> = Vec::with_capacity(slice.len());
        for (i, c) in slice.iter().enumerate() {
            let payload = PointPayload {
                text: c.text.clone(),
                source: c.metadata.source.clone(),
                chunk_id: c.metadata.chunk_id as i64,
                headings: c.metadata.headings.clone(),
                kind: kind.clone(),
                url: url.clone(),
            };
            let vec = vectors
                .get(i)
                .ok_or_else(|| AppError::internal("missing embedding"))?
                .clone();
            points.push((Uuid::new_v4().to_string(), vec, payload));
        }
        let n = points.len();
        upsert_points(&client, points).await?;
        total += n as u64;
    }
    tracing::info!(input = %input, chunks = total, "ingested");
    Ok(IngestStat { chunks: total })
}

pub async fn ingest_paths(inputs: Vec<String>) -> Result<IngestStats> {
    let semaphore = Arc::new(Semaphore::new(2));
    let mut futs = FuturesUnordered::new();
    for inp in inputs {
        let sem = semaphore.clone();
        futs.push(async move {
            let _permit = sem.acquire_owned().await.ok();
            (inp.clone(), ingest_path(&inp).await)
        });
    }

    let mut stats = IngestStats::default();
    while let Some((inp, res)) = futs.next().await {
        match res {
            Ok(s) => {
                stats.ingested += 1;
                stats.chunks += s.chunks;
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!(input = %inp, err = %e, "ingest failed");
            }
        }
    }
    Ok(stats)
}

const PATTERNS: &[&str] = &[
    "pdf", "png", "jpg", "jpeg", "tiff", "bmp", "svg", "drawio", "md", "markdown", "txt", "log",
    "rst", "urls",
];

pub async fn expand_path(target: &str) -> Result<Vec<String>> {
    if is_url(target) {
        return Ok(vec![target.to_string()]);
    }
    let path = PathBuf::from(target);
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| AppError::NotFound(format!("{}: {e}", path.display())))?;

    if meta.is_file() {
        if path.extension().and_then(|e| e.to_str()) == Some("urls") {
            return read_url_list(&path).await;
        }
        return Ok(vec![path.display().to_string()]);
    }

    let mut out: Vec<String> = Vec::new();
    let walker = walkdir::WalkDir::new(&path).follow_links(false);
    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let drawio_svg = name.ends_with(".drawio.svg");
        if !drawio_svg && !PATTERNS.iter().any(|x| **x == ext) {
            continue;
        }
        if ext == "urls" {
            match read_url_list(p).await {
                Ok(urls) => out.extend(urls),
                Err(e) => tracing::warn!(file = %p.display(), err = %e, "url list parse failed"),
            }
        } else {
            out.push(p.display().to_string());
        }
    }
    Ok(out)
}

#[allow(dead_code)]
fn _unused(_: &Path) {}
