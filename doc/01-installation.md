# 01. Installation

`rag-cli` is distributed as a single Rust binary. It requires the Rust toolchain and external services: Qdrant, Ollama, and Docling Serve (for PDF/image/Web URL ingestion).

## Requirements

| Item | Requirement |
|------|-------------|
| OS | Linux (Ubuntu 22.04+ confirmed) / macOS (including Apple Silicon). Windows via WSL only. |
| Rust | stable 1.88+ (pinned by `rust-toolchain.toml`) |
| Cargo | bundled with stable |
| Podman | 4.0+ (includes `podman compose`), or `podman-compose`. Uses the included `docker-compose.yml` directly |
| Ollama | 0.5.x+ (or llama.cpp `llama-server`) |
| RAM | 32 GB recommended (16 GB+ for LLM inference) |
| Storage | SSD 100 GB+ (model cache and Qdrant data) |

## Install the Rust Toolchain

Use the official installer:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

The repository includes a `rust-toolchain.toml`, so stable is selected automatically within the repo.

## Clone the Source

```bash
git clone https://github.com/aquaxis/rag-cli.git
cd rag-cli
```

## Build

### A. Build a local binary

```bash
cargo build --release
# Binary: ./target/release/rag-cli
```

The initial release build takes 1-2 minutes (including onnxruntime prebuilt binary download). The resulting binary is approximately 52 MB.

### B. Install system-wide

```bash
cargo install --path crates/cli
# Binary: ~/.cargo/bin/rag-cli
```

If `~/.cargo/bin` is in your `PATH`, `rag-cli` is available from anywhere.

## Start External Services

### Start Qdrant / Ollama / Docling Serve with Podman Compose

```bash
podman compose up -d
podman compose ps
```

`podman compose` is bundled with Podman 4.0+ (older environments can use the `podman-compose` Python package). The included `docker-compose.yml` filename is recognized as-is. Default ports: Qdrant `127.0.0.1:6333`, Ollama `127.0.0.1:11434`, Docling Serve `127.0.0.1:5001`.

### Pull models into Ollama

```bash
podman exec rag-ollama ollama pull bge-m3
podman exec rag-ollama ollama pull qwen2.5:7b-instruct
podman exec rag-ollama ollama list
```

`bge-m3` (embedding, 1024 dim) and `qwen2.5:7b-instruct` (LLM) are required.

### llama.cpp alternative (optional)

Start `llama-server` on ports 8080 (embeddings) and 8081 (chat) with OpenAI-compatible API, then switch `.env` to `RAG_BACKEND=llamacpp`:

```bash
llama-server -m models/bge-m3-Q5_K_M.gguf --port 8080 --embeddings &
llama-server -m models/qwen2.5-7b-instruct-Q5_K_M.gguf --port 8081 -c 8192 &
```

## Configure Environment Variables

Copy `.env.example` to `.env` and edit:

```bash
cp .env.example .env
$EDITOR .env
```

See [`./05-configuration.md`](./05-configuration.md) for details.

## Verify Installation

```bash
./target/release/rag-cli --version
./target/release/rag-cli status
```

If `status` returns `qdrant: ok / ollama: ok`, installation is complete. Next: [`./02-quickstart.md`](./02-quickstart.md).

---

<- [`./README.md`](./README.md) | -> [`./02-quickstart.md`](./02-quickstart.md)