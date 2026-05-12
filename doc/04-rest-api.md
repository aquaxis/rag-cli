# 04. REST API Reference

The Rust REST API started by `rag-cli serve`. Binds to `127.0.0.1:7777` by default. The primary source is [`crates/api/src/lib.rs`](../crates/api/src/lib.rs).

## Common Specification

- Base URL: `http://127.0.0.1:7777` (overridable via `RAG_API_HOST` and `RAG_API_PORT`)
- Middleware: CORS (permissive), request logger, 15-minute timeout
- Requests/responses use UTF-8 JSON
- On error: HTTP 500 (or 400/404) + `{"error":"<message>"}`
- Request body limit: 100 MB (including `/ingest/upload`)

## Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness check |
| GET | `/status` | Service health + collection list |
| POST | `/ingest` | Ingest path/URL array |
| POST | `/ingest/upload` | Multipart file upload ingestion |
| POST | `/search` | Search + LLM response |
| POST | `/search/stream` | Sources first + LLM token stream |
| POST | `/reindex` | Delete + recreate collection |

## `GET /health`

Liveness check.

Response:

```json
{"status":"ok"}
```

```bash
curl -fsS http://127.0.0.1:7777/health
```

## `GET /status`

Health check for Qdrant / Ollama / Docling Serve and Qdrant collection list.

Response example:

```json
{
  "qdrant": "ok",
  "ollama": "ok",
  "docling": "down",
  "backend": "ollama",
  "collections": [
    { "name": "rag_documents" }
  ]
}
```

```bash
curl -fsS http://127.0.0.1:7777/status | jq
```

## `POST /ingest`

Ingest a mixed array of local paths and/or URLs.

Request:

```json
{
  "paths": ["data/md/note.md", "https://example.com", "data/url/refs.urls"],
  "collection": "rag_documents"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `paths` | `string[]` | Yes | One or more items. Mixed local paths, directories, URLs, and `.urls` files |
| `collection` | `string` | | (Ignored in current version. Reserved for future use.) |

Response:

```json
{
  "ingested": 1,
  "chunks": 1,
  "errors": 0,
  "total": 1
}
```

| Field | Description |
|-------|-------------|
| `ingested` | Number of successfully ingested items |
| `chunks` | Total number of chunks inserted |
| `errors` | Number of failures (logged to stderr) |
| `total` | Total items after expanding `paths` (includes directory recursion and `.urls` expansion) |

```bash
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["data/md/note.md"]}'
```

## `POST /ingest/upload`

Ingest via multipart file upload. Saves to `data/upload/<original_filename>`, then passes through the standard ingestion pipeline.

Request: `multipart/form-data` with a `file` field (required).

Response:

```json
{"path":"data/upload/note.md","chunks":1}
```

```bash
curl -fsS -X POST http://127.0.0.1:7777/ingest/upload \
  -F "file=@data/md/note.md"
```

## `POST /search`

Search + optional LLM response.

Request:

```json
{
  "query": "What is the summary of the sample memo?",
  "top_k": 20,
  "top_n": 3,
  "rerank": true,
  "generate": true
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `query` | `string` | | 1-2000 characters (required) |
| `top_k` | `number` | `TOP_K_RETRIEVE` (20) | Number of retrieval candidates |
| `top_n` | `number` | `TOP_K_RERANK` (5) | Number of results to return |
| `rerank` | `boolean` | `true` | Set `false` to skip reranking |
| `generate` | `boolean` | `true` | Set `false` to skip LLM response generation |

Response:

```json
{
  "answer": "This is a sample memo... [1][2]",
  "sources": [
    {
      "id": "uuid",
      "text": "...",
      "score": 0.6516,
      "rerankScore": 2.146,
      "source": "data/md/note.md",
      "chunkId": 0,
      "headings": ["Summary"],
      "kind": "file"
    }
  ]
}
```

When `generate=false`, `answer` is `null`. When `rerank=false`, `rerankScore` is omitted.

```bash
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"sample memo","top_n":3,"rerank":false,"generate":false}'
```

## `POST /search/stream`

Stream search results and LLM tokens via chunked HTTP body. Request fields are the same as `/search`.

Response format:

```text
{"type":"sources","sources":[...]}\n
---\n
<LLM tokens>...
```

The first line is JSON containing `sources`, the second line is a `---` separator, and subsequent lines are LLM generation tokens.

```bash
curl -N -X POST http://127.0.0.1:7777/search/stream \
  -H 'content-type: application/json' \
  -d '{"query":"sample memo","top_n":3}'
```

Clients should read up to the first `\n` as `sources`, then after `---\n` as `answer`.

## `POST /reindex`

Delete and recreate the Qdrant collection.

Request: no body.

Response:

```json
{"collection":"rag_documents","recreated":true}
```

```bash
curl -fsS -X POST http://127.0.0.1:7777/reindex
```

## Error Responses

| HTTP | Format | Example |
|------|--------|---------|
| 400 | `{"error":"<msg>"}` | Validation failure (empty `paths`, out-of-range `query`, etc.) |
| 404 | `{"error":"<msg>"}` | Ingestion target path does not exist |
| 500 | `{"error":"<msg>"}` | Qdrant / Ollama / Docling communication failure, ort inference failure, etc. |

---

<- [`./03-cli.md`](./03-cli.md) | -> [`./05-configuration.md`](./05-configuration.md)