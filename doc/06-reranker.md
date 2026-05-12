# 06. Reranker (bge-reranker-v2-m3)

The reranking in `rag-cli search` uses `bge-reranker-v2-m3-ONNX` for CPU + fp32 inference. It uses the `ort`, `tokenizers`, and `hf-hub` crates to re-rank results against the existing collection.

## Why a Reranker?

Bi-encoder (`bge-m3`) embedding similarity is fast but does not capture contextual nuances. The cross-encoder bge-reranker-v2-m3 examines the query and passage simultaneously, producing higher-quality scores that effectively reorder the top 5-10 results.

Example: ranking for the query "web ingestion support date"

| Rank | Without reranking | With reranking |
|------|-------------------|----------------|
| 1 | `changelog.txt` | `changelog.txt` (rerank=0.137) |
| 2 | `hello.svg` | `note.md` (rerank=-11.025) |
| 3 | `note.md` (upload) | `note.md` (rerank=-11.025) |
| 4 | `note.md` (md) | `hello.svg` (rerank=-11.035) |

The irrelevant SVG drops from rank 2 to rank 4, while the relevant note moves up.

## Model File Acquisition

On first run, the reranker downloads three files:

| File | Approximate Size |
|------|-------------------|
| `onnx/model.onnx` | ~50 MB |
| `onnx/model.onnx_data` | ~570 MB (external data) |
| `tokenizer.json` | ~10 MB |

Total: ~600 MB. The download path follows this priority:

1. If `RAG_RERANKER_MODEL_DIR=/path` is set, read directly from that directory (no download)
2. Otherwise, download via `hf-hub` crate using `RERANKER_MODEL` (default `onnx-community/bge-reranker-v2-m3-ONNX`)
   - Cache directory: `RAG_HF_CACHE_DIR` or default `~/.cache/huggingface/hub/`

## Offline Operation

Download the model in advance and point `RAG_RERANKER_MODEL_DIR` to it:

```bash
# 1. Download the model in a networked environment (via HF CLI or by running rag-cli search once)
rag-cli search "test" --top-n 1
# -> ~/.cache/huggingface/hub/models--onnx-community--bge-reranker-v2-m3-ONNX/snapshots/<hash>/

# 2. Copy to the offline environment
rsync -av ~/.cache/huggingface/hub/models--onnx-community--bge-reranker-v2-m3-ONNX/snapshots/<hash>/ \
  /opt/rag/reranker/

# 3. Set RAG_RERANKER_MODEL_DIR
export RAG_RERANKER_MODEL_DIR=/opt/rag/reranker
rag-cli search "..." --top-n 3
```

Required layout under `RAG_RERANKER_MODEL_DIR`:

```text
/opt/rag/reranker/
+-- model.onnx
+-- model.onnx_data
+-- tokenizer.json
```

No `onnx/` subdirectory is needed. Copy the three files directly from the snapshot directory (symbolic links also work).

## Performance

| Scenario | Latency |
|----------|---------|
| Cold start (CLI, first run only) | ~2.3 s (including model loading) |
| Warm (API server, subsequent calls) | ~0.4 s (OnceLock + Mutex session reuse) |

Since the CLI process exits after each run, every invocation is a cold start. For batch processing or API usage, run `rag-cli serve` to keep the process alive.

## Batch Size and Memory

| Variable | Default | Description |
|----------|---------|-------------|
| `RAG_RERANK_BATCH` | `8` | Number of passages processed per inference call |

Memory estimates:
- Resident session memory: ~1.5 GB (fp32)
- Inference temporary memory: batch * sequence_length * 2 (i64) * ~3x

If memory is insufficient, reduce `RAG_RERANK_BATCH=1`.

## Skipping Reranking

```bash
rag-cli search "..." --top-n 5 --no-rerank
```

Or via API:

```json
{"query":"...", "top_n":5, "rerank":false}
```

Without reranking, results are still returned by bi-encoder score, which is often sufficient for summarization or approximate search.

## Internal Implementation Overview

- Session is cached globally via `OnceLock<RerankerSession>`
- `Session::run` requires `&mut self`, so the Session is wrapped in `tokio::sync::Mutex`
- Uses `Tensor::from_array((shape, Vec<i64>))` tuple format (avoids ndarray version conflicts)
- Input is `input_ids` + `attention_mask` (XLM-RoBERTa-based, so `token_type_ids` is not needed)
  - If an older BERT-based model is specified via `RERANKER_MODEL`, `Session::inputs()` is checked dynamically for `token_type_ids` presence
- `tokenizer.with_truncation(max_length=512)` + `with_padding(BatchLongest)`
- Output `logits[bs, 1]` is extracted as float32, sorted descending, and `top_n` results are returned

Source: [`crates/search/src/rerank.rs`](../crates/search/src/rerank.rs)

---

<- [`./05-configuration.md`](./05-configuration.md) | -> [`./07-troubleshooting.md`](./07-troubleshooting.md)
