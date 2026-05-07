# rag-cli

ローカル完結のスタンドアロン RAG（Retrieval-Augmented Generation）。Rust 製シングルバイナリで、Hono 互換の REST API（`127.0.0.1:7777`）と CLI を提供する。Node ランタイム不要。

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE.md)

## 特長

- **多形式オフライン取込** — PDF / 画像 / SVG / drawio / Markdown / テキスト / Web URL
- **多言語埋込** — `bge-m3`（1024 dim、Ollama / llama.cpp 切替可）
- **ベクトル検索** — Qdrant v1.12.4 + HNSW Cosine + payload index
- **クロスエンコーダ リランカ** — `bge-reranker-v2-m3-ONNX`（`ort` + `tokenizers` + `hf-hub`）
- **LLM 応答** — `qwen2.5:7b-instruct`（出典 `[1][2]` 付与、不明は明言）
- **日本語チャンキング** — 句読点優先の再帰分割 + 見出しパス前置
- **シングルバイナリ配布** — `cargo install` または `cargo build --release` で完結（Node 不要）

## クイックスタート

```bash
git clone https://github.com/aquaxis/rag-cli.git
cd rag-cli
cargo build --release
docker compose up -d
docker exec rag-ollama ollama pull bge-m3
docker exec rag-ollama ollama pull qwen2.5:7b-instruct
./target/release/rag-cli ingest data/md/note.md
./target/release/rag-cli search "サンプルメモの概要は？" --top-n 3 --no-rerank
```

リランクあり検索は初回のみモデル DL（~600 MB）を伴う。詳細は [`./doc/02-quickstart.md`](./doc/02-quickstart.md)。

## インストール

### A. ローカルでバイナリを生成

```bash
cargo build --release
# → ./target/release/rag-cli
```

### B. システムにインストール

```bash
cargo install --path crates/cli
# → ~/.cargo/bin/rag-cli
```

### C. Docker（任意）

外部サービス（Qdrant / Ollama / Docling Serve）は `docker compose up -d` で起動。`rag-cli` 本体は host で動作する。

詳細手順とトラブルシュートは [`./doc/01-installation.md`](./doc/01-installation.md)。

## 詳細ドキュメント

ユーザ向けの詳細は [`./doc/`](./doc/README.md) を参照。

| ドキュメント | 内容 |
|------------|------|
| [01. インストール](./doc/01-installation.md) | Rust toolchain / Docker / Ollama / llama.cpp の導入 |
| [02. クイックスタート](./doc/02-quickstart.md) | 取込 → 検索 → API 起動の最短手順 |
| [03. CLI リファレンス](./doc/03-cli.md) | 5 サブコマンドの全引数と出力例 |
| [04. REST API リファレンス](./doc/04-rest-api.md) | 7 エンドポイントの仕様と curl 例 |
| [05. 設定](./doc/05-configuration.md) | 環境変数 24 項目と `.env` の使い方 |
| [06. リランカ](./doc/06-reranker.md) | bge-reranker の DL 経路、オフライン化、性能 |
| [07. トラブルシュート](./doc/07-troubleshooting.md) | サービス疎通 / モデル DL / メモリ不足の対処 |
| [08. アーキテクチャ](./doc/08-architecture.md) | 9 クレート構成、データフロー、外部 IF |

## アーキテクチャ概要

```text
ユーザ shell / curl / fetch
        │ HTTP (127.0.0.1:7777)
        ▼
   rag-cli (Rust, single binary)
   ├─ crates/cli       (clap)
   ├─ crates/api       (axum)
   ├─ crates/pipeline  (ingest / retrieve)
   ├─ crates/{ingest,chunk,embed,search,llm,common}
        │
        ▼
   Docling Serve / Ollama / Qdrant / (llama.cpp)
```

詳細は [`./doc/08-architecture.md`](./doc/08-architecture.md) と [`./design_rag.md`](./design_rag.md)。

## 設定の最小例

`.env.example` を `.env` にコピーして編集する。最小の動作確認は既定値のままで可能。

```bash
RAG_BACKEND=ollama
QDRANT_URL=http://127.0.0.1:6333
OLLAMA_HOST=http://127.0.0.1:11434
RAG_API_HOST=127.0.0.1
RAG_API_PORT=7777
```

全環境変数（24 項目）と既定値は [`./doc/05-configuration.md`](./doc/05-configuration.md) に記載。

## CLI

```bash
rag-cli ingest <TARGET>                     # ファイル / ディレクトリ / URL / .urls
rag-cli search <QUERY> [--top-k N] [--top-n N] [--no-rerank] [--no-generate]
rag-cli status
rag-cli reindex
rag-cli serve [--port N]
```

詳細は [`./doc/03-cli.md`](./doc/03-cli.md)。

## REST API

`127.0.0.1:7777` にバインド。エンドポイント:

```text
GET  /health           POST /ingest        POST /search
GET  /status           POST /ingest/upload POST /search/stream
                                            POST /reindex
```

リクエスト / レスポンス JSON は [`./doc/04-rest-api.md`](./doc/04-rest-api.md)。

## バージョン履歴

- **v0.2.1** — リランカ ONNX 本格統合（`ort` + `tokenizers` + `hf-hub`）
- v0.2.0 — Rust 全面移植。Node / pnpm 撤廃
- v0.1.x — TS / Node.js 実装（git 履歴の `v0.1.x` タグから取得可能）

## セキュリティ

- 既定で `127.0.0.1` バインド。LAN / 外部公開時は reverse proxy + 認証を必ず併用すること。
- `data/` 配下と Qdrant snapshot は社内秘扱い。`.gitignore` で除外済み。
- LLM プロンプトに credential を埋めない設計（出典 path のみ）。

## ライセンス

MIT License — see [`./LICENSE.md`](./LICENSE.md).

## 出典 / 参考

- 設計ガイド（一次資料）: [`./design_rag.md`](./design_rag.md)
- [Qdrant](https://qdrant.tech/) / [qdrant-client (Rust)](https://crates.io/crates/qdrant-client)
- [Ollama](https://ollama.com/) / [Docling Serve](https://github.com/docling-project/docling-serve)
- [bge-m3](https://huggingface.co/BAAI/bge-m3) / [bge-reranker-v2-m3-ONNX](https://huggingface.co/onnx-community/bge-reranker-v2-m3-ONNX) / [Qwen2.5](https://huggingface.co/Qwen/Qwen2.5-7B-Instruct)
- [axum](https://github.com/tokio-rs/axum) / [clap](https://github.com/clap-rs/clap) / [tokio](https://tokio.rs/) / [ort](https://ort.pyke.io/) / [hf-hub](https://github.com/huggingface/hf-hub)
