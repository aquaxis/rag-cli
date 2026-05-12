# 05. Configuration

`rag-cli` uses `figment` to merge `.env` + environment variables + defaults. The primary source is [`crates/common/src/config.rs`](../crates/common/src/config.rs).

## Configuration Priority

1. **Environment variables** (highest priority)
2. **`.env` file** (repository root, loaded by `dotenvy`)
3. **Defaults** (`Config`'s `default_*` functions)

`Config` is loaded once at startup via `OnceCell` and is immutable thereafter. The only exception is `--port N` on the CLI, which overrides `RAG_API_PORT`.

## Using `.env`

```bash
cp .env.example .env
$EDITOR .env
```

Even without a `.env` file, all environment variables have defaults, so the application starts.

## Environment Variables

### Qdrant

| Variable | Default | Description |
|----------|---------|-------------|
| `QDRANT_URL` | `http://127.0.0.1:6333` | Qdrant REST endpoint |
| `QDRANT_API_KEY` | (unset) | API key; sent as `api-key` header when set |
| `QDRANT_COLLECTION` | `rag_documents` | Collection name to use |

### Backend Switch

| Variable | Default | Description |
|----------|---------|-------------|
| `RAG_BACKEND` | `ollama` | `ollama` or `llamacpp`. Switches the embedding and LLM backend |

### Ollama

| Variable | Default | Description |
|----------|---------|-------------|
| `OLLAMA_HOST` | `http://127.0.0.1:11434` | Ollama server |
| `OLLAMA_LLM_MODEL` | `qwen2.5:7b-instruct` | LLM model name |
| `OLLAMA_EMBED_MODEL` | `bge-m3` | Embedding model name (dim must match `EMBED_DIM`) |

### llama.cpp (alternative backend, used only when `RAG_BACKEND=llamacpp`)

| Variable | Default | Description |
|----------|---------|-------------|
| `LLAMACPP_EMBED_URL` | `http://127.0.0.1:8080/v1` | OpenAI-compatible embeddings endpoint |
| `LLAMACPP_LLM_URL` | `http://127.0.0.1:8081/v1` | OpenAI-compatible chat endpoint |
| `LLAMACPP_EMBED_MODEL` | `bge-m3` | Embedding model name |
| `LLAMACPP_LLM_MODEL` | `qwen2.5-7b-instruct` | LLM model name |

### Docling Serve

| Variable | Default | Description |
|----------|---------|-------------|
| `DOCLING_URL` | `http://127.0.0.1:5001` | Docling Serve endpoint (used for PDF/image/Web URL ingestion) |

### Reranker

| Variable | Default | Description |
|----------|---------|-------------|
| `RERANKER_MODEL` | `onnx-community/bge-reranker-v2-m3-ONNX` | HuggingFace Hub model ID |
| `RAG_HF_CACHE_DIR` | (unset. Defaults to `~/.cache/huggingface/hub/`) | Override HF Hub cache directory |
| `RAG_RERANKER_MODEL_DIR` | (unset) | When set, skips HF Hub download and reads `model.onnx`, `model.onnx_data`, `tokenizer.json` from this directory |
| `RAG_RERANK_BATCH` | `8` | Reranker inference batch size |

See [`./06-reranker.md`](./06-reranker.md) for details.

### REST API

| Variable | Default | Description |
|----------|---------|-------------|
| `RAG_API_HOST` | `127.0.0.1` | Bind host |
| `RAG_API_PORT` | `7777` | Bind port (overridable via `rag-cli serve --port N`) |

### Chunking

| Variable | Default | Description |
|----------|---------|-------------|
| `CHUNK_SIZE` | `512` | Chunk size limit (in tokens). For Japanese, characters ~= tokens, so `* 3` is applied internally |
| `CHUNK_OVERLAP` | `64` | Chunk overlap (same `* 3` multiplier applied internally) |

### Retrieval

| Variable | Default | Description |
|----------|---------|-------------|
| `TOP_K_RETRIEVE` | `20` | Number of Qdrant Dense search candidates |
| `TOP_K_RERANK` | `5` | Number of results after reranking |
| `EMBED_DIM` | `1024` | Embedding vector dimension (`bge-m3` is 1024; mismatch causes a runtime error) |

### Logging

| Variable | Default | Description |
|----------|---------|-------------|
| `LOG_LEVEL` | `info` | `error` `warn` `info` `debug` `trace` |

`RUST_LOG` is also recognized via `tracing-subscriber::EnvFilter` (more flexible, supports per-module targeting).

## Sample `.env`

```bash
# --- Qdrant ---
QDRANT_URL=http://127.0.0.1:6333
QDRANT_API_KEY=
QDRANT_COLLECTION=rag_documents

# --- Backend ---
RAG_BACKEND=ollama

# --- Ollama ---
OLLAMA_HOST=http://127.0.0.1:11434
OLLAMA_LLM_MODEL=qwen2.5:7b-instruct
OLLAMA_EMBED_MODEL=bge-m3

# --- llama.cpp (optional) ---
LLAMACPP_EMBED_URL=http://127.0.0.1:8080/v1
LLAMACPP_LLM_URL=http://127.0.0.1:8081/v1
LLAMACPP_EMBED_MODEL=bge-m3
LLAMACPP_LLM_MODEL=qwen2.5-7b-instruct

# --- Docling Serve ---
DOCLING_URL=http://127.0.0.1:5001

# --- Reranker ---
RERANKER_MODEL=onnx-community/bge-reranker-v2-m3-ONNX
# RAG_HF_CACHE_DIR=
# RAG_RERANKER_MODEL_DIR=
RAG_RERANK_BATCH=8

# --- REST API ---
RAG_API_HOST=127.0.0.1
RAG_API_PORT=7777

# --- Chunking / Retrieval ---
CHUNK_SIZE=512
CHUNK_OVERLAP=64
TOP_K_RETRIEVE=20
TOP_K_RERANK=5
EMBED_DIM=1024

# --- Logging ---
LOG_LEVEL=info
```

---

<- [`./04-rest-api.md`](./04-rest-api.md) | -> [`./06-reranker.md`](./06-reranker.md)
