# rag-cli

A self-contained, standalone local RAG (Retrieval-Augmented Generation) system. Built as a single Rust binary with an Hono-compatible REST API (`127.0.0.1:7777`) and CLI. No Node.js runtime required.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE.md)

## Features

- **Multi-format offline ingestion** -- PDF / images / SVG / drawio / Markdown / plain text / Web URLs
- **Multilingual embeddings** -- `bge-m3` (1024 dim, switchable between Ollama / llama.cpp)
- **Vector search** -- Qdrant v1.12.4 + HNSW Cosine + payload index
- **Cross-encoder reranker** -- `bge-reranker-v2-m3-ONNX` (`ort` + `tokenizers` + `hf-hub`)
- **LLM response** -- `qwen2.5:7b-instruct` (with source citations `[1][2]`; explicitly states unknowns)
- **Japanese-aware chunking** -- Punctuation-prioritized recursive splitting + heading path prefix
- **Single binary distribution** -- `cargo install` or `cargo build --release` (no Node.js needed)

## Quickstart

```bash
git clone https://github.com/aquaxis/rag-cli.git
cd rag-cli
cargo build --release
podman compose up -d
podman exec rag-ollama ollama pull bge-m3
podman exec rag-ollama ollama pull qwen2.5:7b-instruct
./target/release/rag-cli ingest data/md/note.md
./target/release/rag-cli search "What is the summary of the sample memo?" --top-n 3 --no-rerank
```

The first search with reranking downloads the model (~600 MB). See [`./doc/02-quickstart.md`](./doc/02-quickstart.md).

## Install

### A. Build a local binary

```bash
cargo build --release
# -> ./target/release/rag-cli
```

### B. Install system-wide

```bash
cargo install --path crates/cli
# -> ~/.cargo/bin/rag-cli
```

### C. Podman (optional)

External services (Qdrant / Ollama / Docling Serve) are started with `podman compose up -d`. The `rag-cli` binary runs on the host.

See [`./doc/01-installation.md`](./doc/01-installation.md) for detailed steps and troubleshooting.

## Detailed Documentation

For user-facing details, see [`./doc/`](./doc/README.md).

| Document | Content |
|----------|---------|
| [01. Installation](./doc/01-installation.md) | Rust toolchain / Podman / Ollama / llama.cpp setup |
| [02. Quickstart](./doc/02-quickstart.md) | Ingest -> Search -> API startup in minimal steps |
| [03. CLI Reference](./doc/03-cli.md) | All 5 subcommands with arguments and output examples |
| [04. REST API Reference](./doc/04-rest-api.md) | 7 endpoints with request/response specs and curl examples |
| [05. Configuration](./doc/05-configuration.md) | 24 environment variables and `.env` usage |
| [06. Reranker](./doc/06-reranker.md) | bge-reranker download path, offline setup, performance |
| [07. Troubleshooting](./doc/07-troubleshooting.md) | Service connectivity / model download / OOM solutions |
| [08. Architecture](./doc/08-architecture.md) | 9-crate structure, data flow, external interfaces |

## Architecture Overview

```text
User shell / curl / fetch
        | HTTP (127.0.0.1:7777)
        v
   rag-cli (Rust, single binary)
   |-- crates/cli       (clap)
   |-- crates/api       (axum)
   |-- crates/pipeline  (ingest / retrieve)
   |-- crates/{ingest,chunk,embed,search,llm,common}
        |
        v
   Docling Serve / Ollama / Qdrant / (llama.cpp)
```

See [`./doc/08-architecture.md`](./doc/08-architecture.md) and [`./design_rag.md`](./design_rag.md).

## Minimal Configuration

Copy `.env.example` to `.env` and edit. Default values work for a quick start.

```bash
RAG_BACKEND=ollama
QDRANT_URL=http://127.0.0.1:6333
OLLAMA_HOST=http://127.0.0.1:11434
RAG_API_HOST=127.0.0.1
RAG_API_PORT=7777
```

All 24 environment variables and defaults are in [`./doc/05-configuration.md`](./doc/05-configuration.md).

## CLI

```bash
rag-cli ingest <TARGET>                     # file / directory / URL / .urls
rag-cli search <QUERY> [--top-k N] [--top-n N] [--no-rerank] [--no-generate]
rag-cli status
rag-cli reindex
rag-cli serve [--port N]
```

See [`./doc/03-cli.md`](./doc/03-cli.md).

## REST API

Binds to `127.0.0.1:7777`. Endpoints:

```text
GET  /health           POST /ingest        POST /search
GET  /status           POST /ingest/upload POST /search/stream
                                            POST /reindex
```

Request/response JSON: [`./doc/04-rest-api.md`](./doc/04-rest-api.md).

## Version History

- **v0.2.1** -- Reranker ONNX integration (`ort` + `tokenizers` + `hf-hub`)
- v0.2.0 -- Full Rust port. Node.js / pnpm removed
- v0.1.x -- TypeScript / Node.js implementation (available from `v0.1.x` git tags)

## Security

- Defaults to `127.0.0.1` binding. For LAN/public exposure, always use a reverse proxy with authentication.
- `data/` and Qdrant snapshots are treated as confidential. Excluded via `.gitignore`.
- LLM prompts do not embed credentials (only source paths are included).

## License

MIT License -- see [`./LICENSE.md`](./LICENSE.md).

## Sources / References

- Design guide (primary reference): [`./design_rag.md`](./design_rag.md)
- [Qdrant](https://qdrant.tech/) / [qdrant-client (Rust)](https://crates.io/crates/qdrant-client)
- [Ollama](https://ollama.com/) / [Docling Serve](https://github.com/docling-project/docling-serve)
- [bge-m3](https://huggingface.co/BAAI/bge-m3) / [bge-reranker-v2-m3-ONNX](https://huggingface.co/onnx-community/bge-reranker-v2-m3-ONNX) / [Qwen2.5](https://huggingface.co/Qwen/Qwen2.5-7B-Instruct)
- [axum](https://github.com/tokio-rs/axum) / [clap](https://github.com/clap-rs/clap) / [tokio](https://tokio.rs/) / [ort](https://ort.pyke.io/) / [hf-hub](https://github.com/huggingface/hf-hub)
