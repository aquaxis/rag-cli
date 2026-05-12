# 03. CLI Reference

`rag-cli` is a `clap`-based subcommand CLI. The primary source is [`crates/cli/src/main.rs`](../crates/cli/src/main.rs).

## Overview

```text
rag-cli <COMMAND>

Commands:
  ingest   Ingest file / directory / URL / .urls
  search   Search + LLM response
  status   Service health and collection info
  reindex  Delete and recreate collection
  serve    Start Hono-compatible HTTP API
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

All subcommands support `--help` for details.

## `ingest`

```text
rag-cli ingest <TARGET>
```

| Argument | Required | Description |
|----------|----------|-------------|
| `<TARGET>` | Yes | File / directory / URL / `.urls` file path |

Input type detection:

- Paths starting with `http://` or `https://` are treated as URLs and forwarded to Docling Serve.
- `.urls` files are expanded line by line as URLs (`#` comments and blank lines are skipped).
- Directories are expanded recursively, ingesting only supported extensions (`.pdf` `.png` `.jpg` `.jpeg` `.tiff` `.bmp` `.svg` `.drawio` `.md` `.markdown` `.txt` `.log` `.rst` `.urls`).
- Single files use the converter matching their extension.

Output (single file):

```json
{"chunks":1,"source":"data/md/note.md"}
```

Output (multiple files / directory):

```json
{
  "ingested": 4,
  "chunks": 4,
  "errors": 0,
  "total": 6
}
```

Concurrency is limited to 2 simultaneous tasks (`tokio::sync::Semaphore::new(2)`).

## `search`

```text
rag-cli search <QUERY> [OPTIONS]
```

| Argument | Default | Description |
|----------|---------|-------------|
| `<QUERY>` | | Search query (required, 1-2000 characters) |
| `-k, --top-k <N>` | `20` | Number of retrieval candidates (Qdrant Dense search limit) |
| `-n, --top-n <N>` | `5` | Number of results after reranking |
| `--no-rerank` | off | Skip reranking (return top results by bi-encoder score only) |
| `--no-generate` | off | Disable LLM response generation (show sources only) |

Output:

```text
=== Answer ===
(LLM response; omitted when --no-generate is specified)

=== Sources ===
[1] <source> > <h1> > <h2> (rerank=<score>)
[2] ...
```

`rerank=` shows the reranker score. When `--no-rerank` is specified, it shows `n/a`.

## `status`

```text
rag-cli status
```

No arguments. Returns a health check for Qdrant / Ollama / Docling Serve and the Qdrant collection list as JSON.

```json
{
  "backend": "ollama",
  "collections": [
    { "name": "rag_documents" }
  ],
  "docling": "down",
  "ollama": "ok",
  "qdrant": "ok"
}
```

The `backend` value reflects the current `RAG_BACKEND` setting (`ollama` or `llamacpp`).

## `reindex`

```text
rag-cli reindex
```

No arguments. Deletes and recreates the Qdrant collection `rag_documents` (overridable via `QDRANT_COLLECTION`).

```json
{"collection":"rag_documents","recreated":true}
```

All previously ingested data is lost and must be re-ingested.

## `serve`

```text
rag-cli serve [OPTIONS]
```

| Argument | Default | Description |
|----------|---------|-------------|
| `-p, --port <N>` | `RAG_API_PORT` (default 7777) | API bind port |

Listens on `RAG_API_HOST` (default `127.0.0.1`) and stops on Ctrl-C / SIGINT. See [`./04-rest-api.md`](./04-rest-api.md) for endpoints.

```bash
rag-cli serve --port 7780
# -> Starts on 127.0.0.1:7780
```

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Failure (ingestion, search, or API startup error. `stderr` shows `Error: <message>`) |

## Logging

Logging uses `tracing` and outputs to `stderr`. The level is controlled by the `LOG_LEVEL` environment variable (default `info`).

```bash
LOG_LEVEL=debug rag-cli search "..."
```

See [`./05-configuration.md`](./05-configuration.md) for details.

---

<- [`./02-quickstart.md`](./02-quickstart.md) | -> [`./04-rest-api.md`](./04-rest-api.md)