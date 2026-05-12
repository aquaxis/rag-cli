# 07. Troubleshooting

Quick reference for common issues and their solutions.

## Service Connectivity

### `qdrant: down`

```bash
rag-cli status
# -> "qdrant": "down"
```

Causes and solutions:

| Cause | Solution |
|-------|----------|
| Qdrant is not running | Start with `podman compose up -d qdrant` |
| URL mismatch | Check `QDRANT_URL` (default `http://127.0.0.1:6333`) |
| TLS / authentication required | Set `QDRANT_API_KEY` in `.env` |

```bash
curl -fsS http://127.0.0.1:6333/readyz
# -> "all shards are ready"
```

### `ollama: down`

```bash
rag-cli status
# -> "ollama": "down"
```

| Cause | Solution |
|-------|----------|
| Ollama is not running | `ollama serve` or `podman compose up -d ollama` |
| Model not pulled | `podman exec rag-ollama ollama pull bge-m3` |
| Host mismatch | Check `OLLAMA_HOST` (default `http://127.0.0.1:11434`) |

```bash
curl -fsS http://127.0.0.1:11434/api/tags | jq '.models[].name'
```

### `docling: down`

Only required for PDF/image/Web URL ingestion. SVG/drawio/Markdown/text do not need Docling.

| Cause | Solution |
|-------|----------|
| Docling Serve is not running | `podman compose up -d docling` |
| OOM | Check `podman compose logs docling` for OOM; increase memory or reduce batch size |

## Ingestion Errors

### `Dim mismatch: expected 1024, got N`

| Cause | Solution |
|-------|----------|
| A different embedding model is set in `OLLAMA_EMBED_MODEL` | Set `EMBED_DIM` to match the model, or switch back to bge-m3 |
| `LLAMACPP_EMBED_MODEL` points to a different GGUF model | Check the dimension and update `EMBED_DIM` |

### `Docling errors: ...`

Errors from Docling Serve. Possible causes:

| Cause | Solution |
|-------|----------|
| Corrupted file | Retry with a different PDF/image |
| OOM (large multi-page PDF) | Split pages or reduce concurrency |
| OCR engine not ready | Docling container may still be downloading on first start; check startup logs |

### `Empty markdown from URL: <url>`

The URL was fetched successfully but the Markdown output is empty. Causes:

- Blocked by robots.txt or WAF
- Page requires JavaScript rendering (Docling does not support this)
- Authentication required

This is outside Docling's scope. Download the content manually and ingest as a local file instead.

### Single file fails with `unsupported extension: <ext>`

Supported extensions: `.pdf` `.png` `.jpg` `.jpeg` `.tiff` `.bmp` `.svg` `.drawio` `.drawio.svg` `.md` `.markdown` `.txt` `.log` `.rst` `.urls`. Convert unsupported formats beforehand.

## Reranker Issues

### First run is slow or appears unresponsive

The initial download of bge-reranker-v2-m3-ONNX is ~600 MB. Depending on network bandwidth, this can take tens of seconds to minutes.

```bash
LOG_LEVEL=info rag-cli search "..." --top-n 3 2>&1 | grep "reranker model"
# -> "reranker model files downloaded" indicates download complete
```

### `model.onnx_data has zero size after download`

The download may have been interrupted, leaving a 0-byte file. Delete the cache and retry:

```bash
rm -rf ~/.cache/huggingface/hub/models--onnx-community--bge-reranker-v2-m3-ONNX
rag-cli search "..." --top-n 3
```

### Cannot connect to HuggingFace Hub (offline environment)

After pre-downloading, specify the local directory via `RAG_RERANKER_MODEL_DIR`. See [`./06-reranker.md`](./06-reranker.md) for details.

### OOM killed due to insufficient memory

```bash
RAG_RERANK_BATCH=1 rag-cli search "..." --top-n 3
```

If still insufficient, use `--no-rerank`.

## API Server

### `Address already in use` (port 7777 conflict)

```bash
rag-cli serve --port 7780
# or
RAG_API_PORT=7780 rag-cli serve
```

Use `lsof -i :7777` to check what is using the port.

### CORS prevents browser access

The API allows all origins (`CorsLayer::permissive()`), so this is usually not an issue. If behind a reverse proxy, check the proxy's CORS settings.

## Build / Cargo

### `error: failed to download` / `Network unreachable`

If network restrictions prevent access to crates.io / HF Hub:

```bash
# crates.io: use vendored dependencies
cargo vendor
# Add `replace-with = "vendored-sources"` to .cargo/config.toml
```

### `linker error` / `cannot find -lonnxruntime`

This occurs when the `download-binaries` feature of `ort` is disabled. Ensure `Cargo.toml` workspace.dependencies has `ort` with default features enabled:

```toml
ort = { version = "2.0.0-rc.12" }  # keep default features enabled
```

## Log Level

For detailed logging:

```bash
LOG_LEVEL=debug rag-cli search "..."
# Or per-module:
RUST_LOG=rag_search=debug,rag_pipeline=info rag-cli search "..."
```

Use `LOG_LEVEL=trace` for error details.

---

<- [`./06-reranker.md`](./06-reranker.md) | -> [`./08-architecture.md`](./08-architecture.md)
