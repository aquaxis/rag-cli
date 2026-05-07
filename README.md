# rag-cli — Local Standalone RAG (Rust)

ローカル完結のスタンドアロン RAG（Retrieval-Augmented Generation）サブシステム。
PDF / 画像 / SVG / drawio / Markdown / テキスト / Web URL を取込み、Qdrant ベクトル検索 +
bge-reranker リランク + qwen2.5 LLM で日本語応答を生成する。Rust 製の REST API（`127.0.0.1:7777`）と
`rag-cli` CLI を、シングルバイナリで提供する。

## 特長

- 多形式オフライン取込（Docling Serve + ローカルパーサ）
- 多言語埋込 bge-m3（1024 dim、Ollama / llama.cpp 切替可）
- Qdrant v1.12.4 + HNSW Cosine
- bge-reranker-v2-m3-ONNX による精度向上（PoC 中、現在は `--no-rerank` 推奨）
- qwen2.5:7b-instruct（Ollama / 任意で llama.cpp 副系統）
- 日本語句読点を考慮したチャンキング
- **Node ランタイム不要のシングルバイナリ配布**（`cargo build --release`）

## 動作要件

- Linux (Ubuntu 22.04+) / macOS（Windows は WSL のみ）
- Rust toolchain（stable、`rustc 1.75+`）
- Docker 24.0+ / Docker Compose v2.20+
- Ollama 0.5.x 以上（または llama.cpp `llama-server`）
- RAM 32GB 推奨（LLM 推論時）

## クイックスタート

```bash
git clone https://github.com/aquaxis/rag-cli.git
cd rag-cli
cargo build --release          # target/release/rag-cli が生成される
cp .env.example .env
docker compose up -d
docker exec rag-ollama ollama pull bge-m3
docker exec rag-ollama ollama pull qwen2.5:7b-instruct
./target/release/rag-cli ingest data/md/note.md
./target/release/rag-cli search "サンプルメモの概要は？" --no-rerank
./target/release/rag-cli serve   # Rust API を 127.0.0.1:7777 で起動
```

`cargo install --path crates/cli` で `~/.cargo/bin/rag-cli` に配置することも可能。

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

レスポンスは旧 TS 実装（v0.1.x）と同一スキーマ。詳細仕様は `design_rag.md` を参照。

## CLI

`rag-cli <subcommand>` で以下を提供する。

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

## アーキテクチャ（v0.2 — Rust）

```
ユーザ shell / curl / fetch / 任意HTTPクライアント
        │ HTTP (127.0.0.1:7777)
        ▼
   rag-cli (Rust, single binary)
   ├─ crates/cli       (clap)
   ├─ crates/api       (axum)
   ├─ crates/pipeline  (ingest / retrieve)
   ├─ crates/ingest    (pdf, svg, drawio, md, txt, web)
   ├─ crates/chunk     (日本語句読点チャンキング)
   ├─ crates/embed     (ollama / llamacpp)
   ├─ crates/search    (qdrant / rerank)
   ├─ crates/llm       (ollama / llamacpp)
   └─ crates/common    (config / logger / error)
        │
        ▼
   Docling Serve / Ollama / Qdrant / (llama.cpp)
```

詳細は同梱の `design_rag.md` を参照。

## ビルド / 開発

```bash
cargo build           # dev build
cargo build --release # 配布用バイナリ
cargo test            # 単体テスト
cargo fmt             # フォーマット
cargo clippy          # 静的解析
```

## リランカ（bge-reranker-v2-m3）について

`rag-cli search` のリランクは bge-reranker-v2-m3-ONNX を CPU + fp32 で動かす。
初回起動時に HuggingFace Hub から ~600 MB のモデルファイル（`model.onnx` + `model.onnx_data` + `tokenizer.json`）を取得する。

```bash
# 既定: ~/.cache/huggingface/hub/ に DL（初回のみネットワーク接続必要）
./target/release/rag-cli search "<query>" --top-n 5

# キャッシュ先を上書き
RAG_HF_CACHE_DIR=/path/to/hf-cache ./target/release/rag-cli search "<query>"

# オフライン環境: 事前 DL したディレクトリを直接指定
#   /path/to/dir/{model.onnx, model.onnx_data, tokenizer.json}
RAG_RERANKER_MODEL_DIR=/path/to/dir ./target/release/rag-cli search "<query>"

# リランクをスキップ（bi-encoder の score のみで上位を返す）
./target/release/rag-cli search "<query>" --no-rerank
```

`RAG_RERANK_BATCH`（既定 8）でバッチサイズを調整可。メモリ不足時は 1 に下げる。

セッションは `OnceLock<Mutex<Session>>` でプロセス内キャッシュされるため、`rag-cli serve` で起動した API は 2 回目以降のリクエストで初期化コストを払わない（cold ~2.3s → warm ~0.4s）。

## バージョン履歴

- **v0.2.1** — リランカ ONNX 本格統合（`ort` + `tokenizers` + `hf-hub`）。
- v0.2.0 — Rust 全面移植。Node / pnpm 撤廃。リランカはスタブ。
- v0.1.x — TS / Node.js 実装（git 履歴の `v0.1.x` タグから取得可能）。

## セキュリティ

- 既定で `127.0.0.1` バインド。LAN / 外部公開時は nginx / caddy 等の reverse proxy + 認証を必ず併用すること。
- `data/` 配下と Qdrant snapshot は社内秘扱い。`.gitignore` にて除外済み。
- LLM プロンプトに credential を埋めない設計（出典 path のみ）。

## ライセンス

MIT License — see [LICENSE.md](./LICENSE.md).

## 出典 / 参考

- 設計ガイド: 同梱 `design_rag.md`
- [Docling Serve](https://github.com/docling-project/docling-serve)
- [Qdrant](https://qdrant.tech/) / [qdrant-client (Rust)](https://crates.io/crates/qdrant-client)
- [Ollama](https://ollama.com/)
- [bge-m3](https://huggingface.co/BAAI/bge-m3)
- [bge-reranker-v2-m3-ONNX](https://huggingface.co/onnx-community/bge-reranker-v2-m3-ONNX)
- [Qwen2.5](https://huggingface.co/Qwen/Qwen2.5-7B-Instruct)
- [axum](https://github.com/tokio-rs/axum) / [clap](https://github.com/clap-rs/clap) / [tokio](https://tokio.rs/)
