# 08. Architecture

`rag-cli` is a single binary composed of 9 Cargo workspace crates. For detailed design (protocol specifications, model selection rationale, performance tuning guidelines, etc.), see [`../design_rag.md`](../design_rag.md) as the primary reference. This document provides a summary and the Rust implementation mapping.

## System Overview

```text
+--------------------------------------------------------------------+
|  External clients                                                   |
|   curl / fetch / any HTTP client / shell scripts                    |
+------------------+-------------------------------------------------+
                   | HTTP (127.0.0.1:7777)
                   v
+--------------------------------------------------------------------+
|         rag-cli (Rust, single binary)                               |
|                                                                     |
|   crates/cli       (clap)                                          |
|   crates/api       (axum)                                          |
|   crates/pipeline  (ingest / retrieve)                              |
|   crates/ingest    (pdf, svg, drawio, md, txt, web)                |
|   crates/chunk     (Japanese punctuation-aware chunking)            |
|   crates/embed     (ollama / llamacpp)                              |
|   crates/search    (qdrant / rerank)                               |
|   crates/llm       (ollama / llamacpp)                              |
|   crates/common    (config / logger / error)                        |
+------+-------------+---------------+--------------+----------------+
       |             |               |              |
       v             v               v              v
  Docling Serve   Ollama          Qdrant        ort + tokenizers
  (PDF->MD)      (bge-m3 emb     (vector DB)   (bge-reranker
                  + qwen2.5       collection:    -v2-m3 ONNX)
                  LLM)            rag_documents)
```

## Crate Responsibilities

| Crate | Responsibility | Key External Dependencies |
|-------|---------------|---------------------------|
| `cli` | Subcommand parsing, stdout/stderr formatting, exit codes | `clap`, `tokio`, `anyhow`, `owo-colors` |
| `api` | axum HTTP server, middleware, handlers | `axum`, `tower`, `tower-http`, `serde` |
| `pipeline` | High-level `ingest_path` / `retrieve` orchestration | `tokio`, `futures` |
| `ingest` | Format conversion to Markdown | `reqwest`, `quick-xml`, `flate2`, `serde_yaml` |
| `chunk` | Japanese punctuation-aware recursive chunking | `unicode-segmentation` |
| `embed` | Embedding generation via Ollama / llama.cpp | `reqwest` |
| `search/qdrant` | Qdrant REST client, collection management | `reqwest` |
| `search/rerank` | bge-reranker ONNX inference | `ort`, `tokenizers`, `hf-hub` |
| `llm` | Generation via Ollama / llama.cpp (non-stream + stream) | `reqwest`, `eventsource-stream`, `async-stream` |
| `common` | Configuration (`Config`), logger, shared Error type | `figment`, `tracing`, `thiserror` |

## Data Flow: Ingestion (Offline)

```text
Input (path / URL / .urls)
   |
   v
crates/pipeline::expand_path  -> Vec<Source>
   |
   v
crates/ingest::convert_any
   |-- PDF / image          -> Docling /v1alpha/convert/file       -> Markdown
   |-- SVG / .drawio.svg   -> quick-xml                            -> Markdown
   |-- drawio (.drawio)    -> flate2 + quick-xml                   -> Markdown
   |-- Markdown / text      -> direct read (serde_yaml for frontmatter)
   +-- Web URL             -> Docling /v1alpha/convert/source      -> Markdown
   |
   v
crates/chunk::chunk_japanese  -> Vec<Chunk { text, metadata }>
   |
   v
crates/embed::embed (batch=16/8) -> Vec<Vec<f32>>
   |
   v
crates/search::qdrant::upsert_points (batch=32)
```

Concurrency is limited to 2 simultaneous tasks (`tokio::sync::Semaphore::new(2)` within `ingest_paths`).

## Data Flow: Search (Online)

```text
question (CLI / API)
   |
   v
crates/embed::embed_one -> Vec<f32>
   |
   v
crates/search::qdrant::dense_search (top_k=20) -> Vec<RetrievedDoc>
   |
   v
crates/search::rerank::rerank (top_n=5) -> Vec<RetrievedDoc with score>
   |  (skipped when --no-rerank / rerank=false)
   v
crates/llm::generate / generate_stream -> String / Stream<Item=String>
   |
   v
Output (CLI: stdout, API: JSON or NDJSON-style chunked body)
```

## External Service Dependencies

| Service | Protocol | Purpose | Required |
|---------|----------|---------|----------|
| Qdrant | REST `127.0.0.1:6333` | Vector search / collection management | Required |
| Ollama | REST `127.0.0.1:11434` | Embedding + LLM | Required when `RAG_BACKEND=ollama` |
| llama.cpp | REST OpenAI-compatible `:8080/8081` | Embedding + LLM (alternative) | Required when `RAG_BACKEND=llamacpp` |
| Docling Serve | REST `127.0.0.1:5001` | PDF / image / Web URL to Markdown | Only for PDF/image/URL ingestion |
| HuggingFace Hub | HTTPS | Download bge-reranker-v2-m3-ONNX | First run only (bypassable via `RAG_RERANKER_MODEL_DIR`) |

## Bundled Binary

Release builds bundle the onnxruntime prebuilt binary via the `ort` features. Binary size is approximately 52 MB (`cargo build --release`).

## Concurrency / Performance Parameters

| Parameter | Value | Effect |
|-----------|-------|--------|
| Ingestion concurrency | `Semaphore::new(2)` | 2 simultaneous tasks, limiting Docling / Qdrant load |
| Embedding batch | Ollama 16 / llama.cpp 8 | Reduces API call count |
| Qdrant upsert batch | 32 | Balances payload size and latency per request |
| Reranker batch | `RAG_RERANK_BATCH=8` | Reduces ort inference calls (adjust based on memory) |

## Concurrency / Session Management

- `Config` is loaded once at startup via `OnceCell` (no reload)
- Reranker session is cached globally via `OnceLock<RerankerSession>`, with `Session` wrapped in `tokio::sync::Mutex`
- Qdrant client is created per call, but the internal `reqwest::Client` has connection pooling
- HTTP server (axum) runs on `tokio` multi-thread runtime (`#[tokio::main]`)

## Source Reference Table

| Section | Primary Source Files |
|---------|----------------------|
| Ingestion | [`crates/ingest/src/lib.rs`](../crates/ingest/src/lib.rs), [`crates/ingest/src/{pdf,svg,drawio,markdown,web}.rs`](../crates/ingest/src/) |
| Chunking | [`crates/chunk/src/japanese.rs`](../crates/chunk/src/japanese.rs) |
| Embedding | [`crates/embed/src/{ollama,llamacpp}.rs`](../crates/embed/src/) |
| Qdrant | [`crates/search/src/qdrant.rs`](../crates/search/src/qdrant.rs) |
| Reranker | [`crates/search/src/rerank.rs`](../crates/search/src/rerank.rs) |
| LLM | [`crates/llm/src/{ollama,llamacpp}.rs`](../crates/llm/src/) |
| Pipeline | [`crates/pipeline/src/{ingest,retrieve}.rs`](../crates/pipeline/src/) |
| API | [`crates/api/src/lib.rs`](../crates/api/src/lib.rs) |
| CLI | [`crates/cli/src/main.rs`](../crates/cli/src/main.rs) |
| Config / Logger / Error | [`crates/common/src/{config,logger,error}.rs`](../crates/common/src/) |

## Further Reading

- Primary design reference: [`../design_rag.md`](../design_rag.md) (protocol specifications, model selection, performance tuning guidelines)
- Reranker details: [`./06-reranker.md`](./06-reranker.md)
- Configuration details: [`./05-configuration.md`](./05-configuration.md)

---

<- [`./07-troubleshooting.md`](./07-troubleshooting.md) | -> [`./README.md`](./README.md)