# 02. Quickstart

The shortest path to ingestion, search, and API startup. For details, see [`./03-cli.md`](./03-cli.md) and [`./04-rest-api.md`](./04-rest-api.md).

## Prerequisites

Complete the installation in [`./01-installation.md`](./01-installation.md) and ensure the following are running:

- Qdrant (`127.0.0.1:6333`)
- Ollama (`127.0.0.1:11434`) + `bge-m3` + `qwen2.5:7b-instruct`
- (Optional) Docling Serve (`127.0.0.1:5001`, for PDF/image/Web URL ingestion)

```bash
./target/release/rag-cli status
# -> qdrant: ok / ollama: ok / docling: ok (when Docling is running)
```

## 5 Steps

### 1. Ingest sample data

```bash
./target/release/rag-cli ingest data/md/note.md
# -> {"chunks":1,"source":"data/md/note.md"}
```

Directory ingestion is also supported:

```bash
./target/release/rag-cli ingest data/
# -> {"ingested":4,"chunks":4,"errors":0,"total":6}
```

### 2. Search (without reranking, fastest)

```bash
./target/release/rag-cli search "What is the summary of the sample memo?" --top-n 3 --no-rerank
```

Example output:

```text
=== Answer ===
This is a sample memo document for ingesting Japanese content, searching, and returning top results with reranking.[1][2][3]

=== Sources ===
[1] data/md/note.md > Summary (rerank=n/a)
[2] data/txt/changelog.txt > changelog.txt (rerank=n/a)
[3] data/svg/hello.svg > SVG: hello.svg (rerank=n/a)
```

### 3. Search (with reranking, higher accuracy)

```bash
./target/release/rag-cli search "What is the summary of the sample memo?" --top-n 3
```

The first run downloads bge-reranker-v2-m3-ONNX (~600 MB) from HuggingFace Hub, which takes tens of seconds to minutes. Subsequent runs use the cache. See [`./06-reranker.md`](./06-reranker.md) for details.

### 4. Start the REST API

```bash
./target/release/rag-cli serve &
sleep 2
curl http://127.0.0.1:7777/health
# -> {"status":"ok"}
```

### 5. Search via API

```bash
curl -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"What is the summary of the sample memo?","top_n":3,"rerank":false,"generate":false}'
```

## Comprehensive Ingestion Examples

```bash
./target/release/rag-cli ingest data/md/note.md           # Markdown
./target/release/rag-cli ingest data/txt/changelog.txt    # Plain text
./target/release/rag-cli ingest data/svg/hello.svg        # SVG
./target/release/rag-cli ingest data/drawio/sample.drawio # drawio (no Docling needed)
./target/release/rag-cli ingest data/pdf/sample.pdf       # PDF (requires Docling Serve)
./target/release/rag-cli ingest https://example.com       # Web URL (requires Docling Serve)
./target/release/rag-cli ingest data/url/refs.urls        # .urls file (one URL per line)
./target/release/rag-cli ingest data/                     # Recursive directory
```

## Status and Cleanup

```bash
./target/release/rag-cli status                     # Service + collection overview
./target/release/rag-cli reindex                    # Delete + recreate collection
```

See [`./03-cli.md`](./03-cli.md) for details.

---

<- [`./01-installation.md`](./01-installation.md) | -> [`./03-cli.md`](./03-cli.md)
