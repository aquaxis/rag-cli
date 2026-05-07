# rag-cli — Local Standalone RAG

ローカル完結のスタンドアロン RAG（Retrieval-Augmented Generation）サブシステム。
PDF / 画像 / SVG / drawio / Markdown / テキスト / Web URL を取込み、Qdrant ベクトル検索 +
bge-reranker リランク + qwen2.5 LLM で日本語応答を生成する。Hono REST API（`127.0.0.1:7777`）と
`pnpm rag` CLI を提供する。

## 特長

- 多形式オフライン取込（Docling Serve + ローカルパーサ）
- 多言語埋込 bge-m3（1024 dim）
- Qdrant v1.12.4 + HNSW Cosine
- bge-reranker-v2-m3-ONNX による精度向上
- qwen2.5:7b-instruct（Ollama / 任意で llama.cpp 副系統）
- 日本語句読点を考慮したチャンキング

## 動作要件

- Linux (Ubuntu 22.04+) / macOS
- Node.js 20.x 以上、pnpm 9.x 以上
- Docker 24.0+ / Docker Compose v2.20+
- Ollama 0.5.x 以上（または llama.cpp `llama-server`）
- RAM 32GB 推奨（LLM 推論時）

## クイックスタート

```bash
git clone https://github.com/aquaxis/rag-cli.git
cd rag-cli
pnpm install
cp .env.example .env
docker compose up -d
docker exec rag-ollama ollama pull bge-m3
docker exec rag-ollama ollama pull qwen2.5:7b-instruct
pnpm rag ingest data/md/note.md
pnpm rag search "サンプルメモの概要は？"
pnpm serve   # Hono API を 127.0.0.1:7777 で起動
```

## REST API

既定で `127.0.0.1:7777` にバインドする。

| メソッド | パス               | 用途                                             |
|---------|--------------------|--------------------------------------------------|
| GET     | `/health`          | 死活確認（`{status:"ok"}`）                      |
| GET     | `/status`          | Qdrant / Ollama / Docling のヘルス + collections |
| POST    | `/ingest`          | パス / URL 配列を取込（混在可）                  |
| POST    | `/ingest/upload`   | multipart ファイルアップロード取込               |
| POST    | `/search`          | 検索 + LLM 応答（`generate=false` で出典のみ）   |
| POST    | `/search/stream`   | 出典先送り + LLM トークンストリーム              |
| POST    | `/reindex`         | collection 削除 + 再作成                         |

リクエスト / レスポンスの詳細仕様は `design_rag.md` を参照。

## CLI

`pnpm rag <subcommand>` で以下を提供する。

- `ingest <target>` — ファイル / ディレクトリ / URL / `.urls` を受理
- `search <query> [--top-k N] [--top-n N] [--no-rerank] [--no-generate]`
- `status`
- `reindex`
- `serve [--port N]`

## 設定

`.env.example` を `.env` にコピーして編集する。主要変数:

- `RAG_BACKEND=ollama|llamacpp`
- `OLLAMA_HOST=http://127.0.0.1:11434`
- `OLLAMA_LLM_MODEL=qwen2.5:7b-instruct`
- `OLLAMA_EMBED_MODEL=bge-m3`
- `QDRANT_URL=http://127.0.0.1:6333`
- `QDRANT_COLLECTION=rag_documents`
- `DOCLING_URL=http://127.0.0.1:5001`
- `RAG_API_HOST=127.0.0.1`
- `RAG_API_PORT=7777`
- `CHUNK_SIZE=512` / `CHUNK_OVERLAP=64`
- `TOP_K_RETRIEVE=20` / `TOP_K_RERANK=5`
- `EMBED_DIM=1024`

詳細は `.env.example` および `design_rag.md` §4.5 を参照。

## アーキテクチャ

```
ユーザ shell / curl / fetch / 任意HTTPクライアント
        │ HTTP (127.0.0.1:7777)
        ▼
   Hono API (Node.js / pnpm rag)
        │
        ├─ Docling Serve  (PDF/画像/Web URL → Markdown)
        ├─ Ollama         (bge-m3 埋込 + qwen2.5 LLM)
        ├─ Qdrant         (vector DB / collection: rag_documents)
        └─ transformers.js (bge-reranker-v2-m3 リランク)
```

詳細は同梱の `design_rag.md` を参照。

## セキュリティ

- 既定で `127.0.0.1` バインド。LAN / 外部公開時は nginx / caddy 等の reverse proxy + 認証を必ず併用すること。
- `data/` 配下と Qdrant snapshot は社内秘扱い。`.gitignore` にて除外済み。
- LLM プロンプトに credential を埋めない設計（出典 path のみ）。

## ライセンス

MIT License — see [LICENSE.md](./LICENSE.md).

## 出典 / 参考

- 設計ガイド: 同梱 `design_rag.md`
- [Docling Serve](https://github.com/docling-project/docling-serve)
- [Qdrant](https://qdrant.tech/)
- [Ollama](https://ollama.com/)
- [bge-m3](https://huggingface.co/BAAI/bge-m3)
- [bge-reranker-v2-m3-ONNX](https://huggingface.co/onnx-community/bge-reranker-v2-m3-ONNX)
- [Qwen2.5](https://huggingface.co/Qwen/Qwen2.5-7B-Instruct)
