# 05. 設定

`rag-cli` は `figment` を使って `.env` + 環境変数 + 既定値をマージする。一次ソースは [`crates/common/src/config.rs`](../crates/common/src/config.rs)。

## 設定の優先順位

1. **環境変数**（最優先）
2. **`.env` ファイル**（リポジトリ ルート、`dotenvy` で読込）
3. **既定値**（`Config` の `default_*` 関数）

`Config` は `OnceCell` で起動直後にロードされ、以後は不変。CLI で `--port N` を指定した場合のみ `RAG_API_PORT` を上書きする例外がある。

## `.env` の使い方

```bash
cp .env.example .env
$EDITOR .env
```

`.env` が存在しない場合でも、すべての環境変数に既定値があるため起動は可能。

## 環境変数一覧

### Qdrant

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `QDRANT_URL` | `http://127.0.0.1:6333` | Qdrant REST エンドポイント |
| `QDRANT_API_KEY` | （未設定） | API key、設定時は `api-key` ヘッダで送信 |
| `QDRANT_COLLECTION` | `rag_documents` | 使用 collection 名 |

### Backend 切替

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `RAG_BACKEND` | `ollama` | `ollama` または `llamacpp`。embed と LLM の経路を切替 |

### Ollama

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `OLLAMA_HOST` | `http://127.0.0.1:11434` | Ollama サーバ |
| `OLLAMA_LLM_MODEL` | `qwen2.5:7b-instruct` | LLM モデル名 |
| `OLLAMA_EMBED_MODEL` | `bge-m3` | 埋込モデル名（dim は `EMBED_DIM` と一致必須） |

### llama.cpp（副系統、`RAG_BACKEND=llamacpp` 時のみ使用）

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `LLAMACPP_EMBED_URL` | `http://127.0.0.1:8080/v1` | OpenAI 互換 embeddings エンドポイント |
| `LLAMACPP_LLM_URL` | `http://127.0.0.1:8081/v1` | OpenAI 互換 chat エンドポイント |
| `LLAMACPP_EMBED_MODEL` | `bge-m3` | 埋込モデル名 |
| `LLAMACPP_LLM_MODEL` | `qwen2.5-7b-instruct` | LLM モデル名 |

### Docling Serve

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `DOCLING_URL` | `http://127.0.0.1:5001` | Docling Serve エンドポイント（PDF / 画像 / Web URL 取込で使用） |

### Reranker

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `RERANKER_MODEL` | `onnx-community/bge-reranker-v2-m3-ONNX` | HuggingFace Hub のモデル ID |
| `RAG_HF_CACHE_DIR` | （未設定。`~/.cache/huggingface/hub/`） | HF Hub キャッシュディレクトリの上書き |
| `RAG_RERANKER_MODEL_DIR` | （未設定） | 設定時は HF Hub 経由 DL をスキップし、このディレクトリから `model.onnx` `model.onnx_data` `tokenizer.json` を読込む |
| `RAG_RERANK_BATCH` | `8` | リランカ推論のバッチサイズ |

詳細は [`./06-reranker.md`](./06-reranker.md)。

### REST API

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `RAG_API_HOST` | `127.0.0.1` | バインドホスト |
| `RAG_API_PORT` | `7777` | バインドポート（`rag-cli serve --port N` で上書き可） |

### チャンキング

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `CHUNK_SIZE` | `512` | チャンク上限（トークン換算）。日本語は文字 ≒ トークンで `* 3` 倍を内部適用 |
| `CHUNK_OVERLAP` | `64` | チャンク間 overlap（同上、`* 3` 倍を内部適用） |

### 検索

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `TOP_K_RETRIEVE` | `20` | Qdrant Dense 検索の候補数 |
| `TOP_K_RERANK` | `5` | rerank 後の最終出力数 |
| `EMBED_DIM` | `1024` | 埋込ベクトルの次元（`bge-m3` は 1024、不一致は実行時エラー） |

### ロギング

| 環境変数 | 既定値 | 説明 |
|---------|--------|------|
| `LOG_LEVEL` | `info` | `error` `warn` `info` `debug` `trace` |

`RUST_LOG` も `tracing-subscriber::EnvFilter` 経由で認識される（より柔軟、モジュール別の指定が可能）。

## サンプル `.env`

```bash
# ─── Qdrant ───
QDRANT_URL=http://127.0.0.1:6333
QDRANT_API_KEY=
QDRANT_COLLECTION=rag_documents

# ─── Backend ───
RAG_BACKEND=ollama

# ─── Ollama ───
OLLAMA_HOST=http://127.0.0.1:11434
OLLAMA_LLM_MODEL=qwen2.5:7b-instruct
OLLAMA_EMBED_MODEL=bge-m3

# ─── llama.cpp（任意）───
LLAMACPP_EMBED_URL=http://127.0.0.1:8080/v1
LLAMACPP_LLM_URL=http://127.0.0.1:8081/v1
LLAMACPP_EMBED_MODEL=bge-m3
LLAMACPP_LLM_MODEL=qwen2.5-7b-instruct

# ─── Docling Serve ───
DOCLING_URL=http://127.0.0.1:5001

# ─── Reranker ───
RERANKER_MODEL=onnx-community/bge-reranker-v2-m3-ONNX
# RAG_HF_CACHE_DIR=
# RAG_RERANKER_MODEL_DIR=
RAG_RERANK_BATCH=8

# ─── REST API ───
RAG_API_HOST=127.0.0.1
RAG_API_PORT=7777

# ─── チャンキング・検索 ───
CHUNK_SIZE=512
CHUNK_OVERLAP=64
TOP_K_RETRIEVE=20
TOP_K_RERANK=5
EMBED_DIM=1024

# ─── ロギング ───
LOG_LEVEL=info
```

---

← [`./04-rest-api.md`](./04-rest-api.md) | → [`./06-reranker.md`](./06-reranker.md)
