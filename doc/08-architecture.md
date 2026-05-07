# 08. アーキテクチャ

`rag-cli` は Cargo workspace の 9 クレートで構成される単一バイナリ。詳細な設計（プロトコル仕様、モデル選定理由、性能チューニング指針など）は [`../design_rag.md`](../design_rag.md) を一次資料とする。本書はそのサマリと Rust 実装の対応関係を示す。

## システム全体図

```text
┌────────────────────────────────────────────────────────────────────┐
│  外部クライアント                                                    │
│   curl / fetch / 任意 HTTP クライアント / シェルスクリプト           │
└──────────────────┬─────────────────────────────────────────────────┘
                   │ HTTP (127.0.0.1:7777)
                   ▼
┌────────────────────────────────────────────────────────────────────┐
│         rag-cli (Rust, single binary)                              │
│                                                                     │
│   crates/cli       (clap)                                           │
│   crates/api       (axum)                                           │
│   crates/pipeline  (ingest / retrieve)                              │
│   crates/ingest    (pdf, svg, drawio, md, txt, web)                 │
│   crates/chunk     (日本語句読点チャンキング)                        │
│   crates/embed     (ollama / llamacpp)                              │
│   crates/search    (qdrant / rerank)                                │
│   crates/llm       (ollama / llamacpp)                              │
│   crates/common    (config / logger / error)                        │
└──────┬──────────────┬───────────────┬──────────────┬────────────────┘
       │              │               │              │
       ▼              ▼               ▼              ▼
  Docling Serve   Ollama          Qdrant        ort + tokenizers
  (PDF→MD)        (bge-m3 emb     (vector DB)   (bge-reranker
                   + qwen2.5       collection:    -v2-m3 ONNX)
                   LLM)            rag_documents)
```

## クレート責務

| クレート | 責務 | 主要外部依存 |
|---------|------|--------------|
| `cli` | サブコマンド解釈、stdout/stderr 整形、終了コード | `clap`, `tokio`, `anyhow`, `owo-colors` |
| `api` | axum HTTP サーバ、ミドルウェア、ハンドラ | `axum`, `tower`, `tower-http`, `serde` |
| `pipeline` | `ingest_path` / `retrieve` の高レベル制御 | `tokio`, `futures` |
| `ingest` | 各種フォーマット変換 → Markdown | `reqwest`, `quick-xml`, `flate2`, `serde_yaml` |
| `chunk` | 日本語句読点優先の再帰チャンキング | `unicode-segmentation` |
| `embed` | Ollama / llama.cpp 経由の埋込生成 | `reqwest` |
| `search/qdrant` | Qdrant REST クライアント、collection 管理 | `reqwest` |
| `search/rerank` | bge-reranker ONNX 推論 | `ort`, `tokenizers`, `hf-hub` |
| `llm` | Ollama / llama.cpp での生成（非ストリーム + ストリーム） | `reqwest`, `eventsource-stream`, `async-stream` |
| `common` | 設定（`Config`）、ロガー、共通 Error 型 | `figment`, `tracing`, `thiserror` |

## データフロー: 取込（オフライン）

```text
入力（パス / URL / .urls）
   │
   ▼
crates/pipeline::expand_path  → Vec<Source>
   │
   ▼
crates/ingest::convert_any
   ├─ PDF / 画像          → Docling /v1alpha/convert/file       → Markdown
   ├─ SVG / .drawio.svg   → quick-xml                            → Markdown
   ├─ drawio (.drawio)    → flate2 + quick-xml                   → Markdown
   ├─ Markdown / テキスト → 直接読込（serde_yaml で frontmatter）
   └─ Web URL             → Docling /v1alpha/convert/source      → Markdown
   │
   ▼
crates/chunk::chunk_japanese  → Vec<Chunk { text, metadata }>
   │
   ▼
crates/embed::embed (batch=16/8) → Vec<Vec<f32>>
   │
   ▼
crates/search::qdrant::upsert_points (batch=32)
```

並列度は `tokio::sync::Semaphore::new(2)`（ingest_paths 内）。

## データフロー: 検索（オンライン）

```text
question (CLI / API)
   │
   ▼
crates/embed::embed_one → Vec<f32>
   │
   ▼
crates/search::qdrant::dense_search (top_k=20) → Vec<RetrievedDoc>
   │
   ▼
crates/search::rerank::rerank (top_n=5) → Vec<RetrievedDoc with score>
   │  （--no-rerank / rerank=false ならスキップ）
   ▼
crates/llm::generate / generate_stream → String / Stream<Item=String>
   │
   ▼
出力（CLI: stdout、API: JSON または NDJSON 風 chunked body）
```

## 外部 IF への依存

| サービス | プロトコル | 用途 | 必須性 |
|---------|-----------|------|--------|
| Qdrant | REST `127.0.0.1:6333` | ベクトル検索 / collection 管理 | 必須 |
| Ollama | REST `127.0.0.1:11434` | 埋込 + LLM | `RAG_BACKEND=ollama` 時必須 |
| llama.cpp | REST OpenAI 互換 `:8080/8081` | 埋込 + LLM（副系統） | `RAG_BACKEND=llamacpp` 時必須 |
| Docling Serve | REST `127.0.0.1:5001` | PDF / 画像 / Web URL → Markdown | PDF / 画像 / URL 取込時のみ |
| HuggingFace Hub | HTTPS | bge-reranker-v2-m3-ONNX の DL | 初回起動時のみ（`RAG_RERANKER_MODEL_DIR` でバイパス可） |

## 同梱バイナリ

リリースビルドは onnxruntime のプリビルドバイナリを `ort` features 経由で同梱する。バイナリサイズは約 52 MB（`cargo build --release`）。

## 並行制御 / 性能パラメータ

| パラメータ | 値 | 効果 |
|-----------|-----|------|
| 取込並列度 | `Semaphore::new(2)` | 同時 2 件、Docling / Qdrant の負荷を抑制 |
| 埋込バッチ | Ollama 16 / llama.cpp 8 | API 呼出回数削減 |
| Qdrant upsert バッチ | 32 | 1 リクエストの payload サイズと latency のバランス |
| Reranker バッチ | `RAG_RERANK_BATCH=8` | ort 推論コール削減（メモリと相談） |

## 並走 / セッション管理

- `Config` は `OnceCell` で起動直後にロード（再読込なし）
- Reranker session は `OnceLock<RerankerSession>` でグローバル cache、`Session` は `tokio::sync::Mutex` で wrap
- Qdrant client は呼出ごとに作るが、内部の `reqwest::Client` は connection pool を持つ
- HTTP server（axum）は `tokio` の multi-thread runtime（`#[tokio::main]`）で動作

## ソース対応表

| 章 | 主な参照ファイル |
|----|-----------------|
| 取込 | [`crates/ingest/src/lib.rs`](../crates/ingest/src/lib.rs)、[`crates/ingest/src/{pdf,svg,drawio,markdown,web}.rs`](../crates/ingest/src/) |
| チャンキング | [`crates/chunk/src/japanese.rs`](../crates/chunk/src/japanese.rs) |
| 埋込 | [`crates/embed/src/{ollama,llamacpp}.rs`](../crates/embed/src/) |
| Qdrant | [`crates/search/src/qdrant.rs`](../crates/search/src/qdrant.rs) |
| リランカ | [`crates/search/src/rerank.rs`](../crates/search/src/rerank.rs) |
| LLM | [`crates/llm/src/{ollama,llamacpp}.rs`](../crates/llm/src/) |
| パイプライン | [`crates/pipeline/src/{ingest,retrieve}.rs`](../crates/pipeline/src/) |
| API | [`crates/api/src/lib.rs`](../crates/api/src/lib.rs) |
| CLI | [`crates/cli/src/main.rs`](../crates/cli/src/main.rs) |
| 設定 / ロガー / Error | [`crates/common/src/{config,logger,error}.rs`](../crates/common/src/) |

## さらに詳しく

- 一次設計資料: [`../design_rag.md`](../design_rag.md)（プロトコル仕様、モデル選定、性能チューニング指針）
- リランカの詳細: [`./06-reranker.md`](./06-reranker.md)
- 設定詳細: [`./05-configuration.md`](./05-configuration.md)

---

← [`./07-troubleshooting.md`](./07-troubleshooting.md) | → [`./README.md`](./README.md)
