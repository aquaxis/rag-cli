.PHONY: help install up down logs pull-models typecheck serve rag status reindex

help:
	@echo "Local Standalone RAG — make targets"
	@echo ""
	@echo "  install      pnpm install"
	@echo "  up           docker compose up -d"
	@echo "  down         docker compose stop"
	@echo "  logs         docker compose logs -f"
	@echo "  pull-models  pull bge-m3 + qwen2.5:7b-instruct into Ollama"
	@echo "  typecheck    pnpm typecheck"
	@echo "  serve        pnpm serve"
	@echo "  rag          pnpm rag (pass args via ARGS=)"
	@echo "  status       pnpm rag status"
	@echo "  reindex      pnpm rag reindex"

install:
	pnpm install

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

typecheck:
	pnpm typecheck

serve:
	pnpm serve

rag:
	pnpm rag $(ARGS)

status:
	pnpm rag status

reindex:
	pnpm rag reindex
