.PHONY: help build release test fmt clippy run install up down logs pull-models serve status reindex

help:
	@echo "rag-cli (Rust) — make targets"
	@echo ""
	@echo "  build        cargo build (dev)"
	@echo "  release      cargo build --release"
	@echo "  test         cargo test"
	@echo "  fmt          cargo fmt"
	@echo "  clippy       cargo clippy -- -D warnings"
	@echo "  install      cargo install --path crates/cli"
	@echo "  up           docker compose up -d"
	@echo "  down         docker compose stop"
	@echo "  logs         docker compose logs -f"
	@echo "  pull-models  pull bge-m3 + qwen2.5:7b-instruct into Ollama"
	@echo "  serve        ./target/release/rag-cli serve"
	@echo "  status       ./target/release/rag-cli status"
	@echo "  reindex      ./target/release/rag-cli reindex"

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings

install:
	cargo install --path crates/cli

up:
	docker compose up -d

down:
	docker compose stop

logs:
	docker compose logs -f

pull-models:
	docker exec rag-ollama ollama pull bge-m3
	docker exec rag-ollama ollama pull qwen2.5:7b-instruct
	docker exec rag-ollama ollama list

serve:
	./target/release/rag-cli serve

status:
	./target/release/rag-cli status

reindex:
	./target/release/rag-cli reindex
