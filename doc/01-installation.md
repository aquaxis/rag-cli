# 01. インストール

`rag-cli` は単一の Rust バイナリとして配布される。動作には Rust toolchain と、外部サービスとして Qdrant / Ollama / Docling Serve（PDF / 画像 / Web URL を取込む場合）が必要となる。

## 動作環境

| 項目 | 要件 |
|------|------|
| OS | Linux (Ubuntu 22.04+ を確認) / macOS（Apple Silicon を含む）。Windows は WSL のみ。 |
| Rust | stable 1.88 以上（`rust-toolchain.toml` で固定） |
| Cargo | stable 同梱 |
| Docker | 24.0+, Docker Compose v2.20+（外部サービス起動用） |
| Ollama | 0.5.x 以上（または llama.cpp `llama-server`） |
| RAM | 32 GB 推奨（LLM 推論時 16 GB 以上） |
| ストレージ | SSD 100 GB 以上（モデルキャッシュと Qdrant データ用） |

## Rust toolchain の導入

公式インストーラを使う:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

`rust-toolchain.toml` がリポジトリに含まれているため、リポジトリ内では自動的に stable が選択される。

## ソースの取得

```bash
git clone https://github.com/aquaxis/rag-cli.git
cd rag-cli
```

## ビルド方法

### A. ローカルでバイナリを生成

```bash
cargo build --release
# バイナリ: ./target/release/rag-cli
```

リリースビルドは初回 1〜2 分かかる（onnxruntime のプリビルドバイナリ DL を含む）。生成されるバイナリサイズは約 52 MB。

### B. システムにインストール

```bash
cargo install --path crates/cli
# バイナリ: ~/.cargo/bin/rag-cli
```

`~/.cargo/bin` が `PATH` に含まれていれば、どこからでも `rag-cli` が呼べる。

## 外部サービスの起動

### Docker Compose で Qdrant / Ollama / Docling Serve を起動

```bash
docker compose up -d
docker compose ps
```

ポートの既定: Qdrant `127.0.0.1:6333`、Ollama `127.0.0.1:11434`、Docling Serve `127.0.0.1:5001`。

### Ollama にモデルを投入

```bash
docker exec rag-ollama ollama pull bge-m3
docker exec rag-ollama ollama pull qwen2.5:7b-instruct
docker exec rag-ollama ollama list
```

`bge-m3`（埋込、1024 dim）と `qwen2.5:7b-instruct`（LLM）が必要。

### llama.cpp 副系統（任意）

OpenAI 互換 API として `llama-server` を 8080（embeddings）と 8081（chat）で起動し、`.env` で `RAG_BACKEND=llamacpp` に切替える:

```bash
llama-server -m models/bge-m3-Q5_K_M.gguf --port 8080 --embeddings &
llama-server -m models/qwen2.5-7b-instruct-Q5_K_M.gguf --port 8081 -c 8192 &
```

## 環境変数の設定

`.env.example` を `.env` にコピーして編集する:

```bash
cp .env.example .env
$EDITOR .env
```

詳細は [`./05-configuration.md`](./05-configuration.md) を参照。

## 動作確認

```bash
./target/release/rag-cli --version
./target/release/rag-cli status
```

`status` が `qdrant: ok / ollama: ok` を返せばインストール完了。次は [`./02-quickstart.md`](./02-quickstart.md) へ。

---

← [`./README.md`](./README.md) | → [`./02-quickstart.md`](./02-quickstart.md)
