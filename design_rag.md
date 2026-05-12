# Standalone RAG Environment Setup Guide

> **Target audience**: Engineers who want to build a standalone RAG subsystem that runs entirely on a local environment
> **Last updated**: 2026-05-06
> **Location**: Anywhere (referenced as `./design_rag.md`)
> **Reference document**: `./pdf_image_rag_guide_nodejs.md` (1640 lines, the foundation of this guide)

---

## TL;DR (Key points in under 30 lines)

- A single Markdown guide to build a **locally-contained standalone RAG subsystem** (project directory: `rag-system/`).
- Stack: **Node.js 20+ / pnpm 9+ / Docker Compose v2 / Qdrant / Ollama (or llama.cpp) / Docling Serve / Hono / transformers.js**. Zero dependency on external SaaS.
- Ingestion supports **PDF / images / SVG / drawio / Markdown / plain text / Web (URL)**. SVG and drawio use the newly added XML parser + label extraction path; Web uses Docling Serve's `/v1alpha/convert/source` URL input path.
- External applications / CLI / shell scripts can access the Hono REST API at `127.0.0.1:7777`.
- Users can access all features from the shell via `pnpm rag {ingest|search|status|reindex|serve} ...`.
- Japanese support: multilingual `bge-m3` embeddings (1024 dim), `qwen2.5:7b-instruct` LLM, Japanese-enhanced chunking with `、。！？` as priority separators.
- This guide is structured as **inherit + override + append** from the reference guide `pdf_image_rag_guide_nodejs.md`, with verification commands and expected results at the end of each section.

---

## 0. Relationship to the Reference Guide

### 0.1 Inherit / Override / Append Comparison Table

| Section | Content | Relationship to Reference Guide |
|----------|---------|---------------------------------|
| 1 | Architecture overview | Partially overridden |
| 2 | Requirements | Inherited + llama.cpp path added |
| 3 | Directory layout (contained within `rag-system/`) | **New** |
| 4 | Phase 0: Project initialization | Inherited (pnpm / TypeScript / .gitignore) |
| 5 | Phase 1: Docker Compose | Inherited |
| 6 | Phase 2: Document conversion (**SVG / drawio / Markdown / text / Web added**) | Partially new |
| 7 | Phase 3: Chunking (Japanese-enhanced) | Partially overridden |
| 8 | Phase 4: Embedding generation (**llama.cpp path added**) | Partially new |
| 9 | Phase 5: Qdrant ingestion | Inherited |
| 10 | Phase 6: Search + reranking | Inherited |
| 11 | Phase 7: Local LLM response (**llama.cpp path added**) | Partially new |
| 12 | Phase 8: HTTP API (Hono) | Partially overridden |
| 13 | Phase 9: User CLI (`pnpm rag` subcommands) | **New** |
| 14 | Phase 10: External application / client usage examples | **New** |
| 15 | Phase 11: Operations / Troubleshooting | Inherited |
| 16 | Appendix (alternative stack / performance / security) | Partially overridden |

### 0.2 Position of This Guide

The reference guide `pdf_image_rag_guide_nodejs.md` is a **general-purpose** PDF/image RAG construction procedure. This guide adapts it for standalone operation and **always adds** the following:

1. **SVG / drawio / Markdown / plain text / Web (URL)** as ingestion formats
2. **llama.cpp** as an alternative inference backend alongside Ollama (for environments where Ollama doesn't work)
3. **HTTP REST API** (Hono) for external applications / CLI / any client
4. **User CLI** (`pnpm rag`) for direct shell operation
5. **Japanese enhancement** (punctuation `。、！？` as priority separators, qwen2.5 as default)

When referencing sections/code from the reference guide, use **section numbers** and maintain a granularity that allows this guide to be self-contained.

---

## 1. Architecture Overview

### 1.1 Overall Diagram

```
+----------------------------------------------------------------------+
|  External clients (any)                                               |
|   - User shell / CLI / Web front / other apps / shell scripts          |
|   - curl / fetch / SDK for HTTP                                       |
+------------------+---------------------------------------------------+
                   | HTTP (127.0.0.1:7777)
                   v
+----------------------------------------------------------------------+
|         Hono API (Node.js / pnpm -- rag-system/)                       |
|   POST /ingest    POST /search    GET /status    POST /reindex        |
|                                                                       |
|   pnpm rag {ingest|search|status|reindex|serve}                       |
+------+-------------+---------------+---------------+-----------------+
       |             |               |               |
       v             v               v               v
  Docling Serve   Ollama          Qdrant         transformers.js
  (PDF->MD)      (bge-m3 emb     (vector DB)    (bge-reranker-v2-m3)
                  + qwen2.5
                  LLM)            collection:
                                  rag_documents
       |
       | <- llama.cpp (llama-server) can run alongside Ollama as an alternative
       v
   /v1/chat/completions / /v1/embeddings (OpenAI-compatible)
```

### 1.2 Data Flow

**Ingestion (offline)**:
```
  PDF / image / SVG / drawio / Markdown / text / Web URL
     |
     v
  Format detection (extension / mime / URL scheme)
     |
     |-- PDF / image          -> Docling Serve (REST /convert/file) -> Markdown
     |-- SVG / .drawio.svg   -> fast-xml-parser -> text/title/desc extraction -> Markdown
     |-- drawio (.drawio)    -> pako.inflateRaw -> mxGraph XML -> mxCell.value extraction -> Markdown
     |-- Markdown / text      -> direct read (frontmatter removal if needed)
     +-- Web URL (http(s)://) -> Docling Serve (REST /convert/source URL) -> Markdown
     |
     v
  Japanese-enhanced chunking (punctuation separators)
     |
     v
  Ollama bge-m3 (or llama.cpp embeddings) -> 1024 dim Dense vectors
     |
     v
  Qdrant upsert (collection: rag_documents)
```

**Search (online)**:
```
  User question (CLI / Hono /search / any client)
     |
     v
  bge-m3 query embedding
     |
     v
  Qdrant Dense search (top_k=20)
     |
     v
  transformers.js bge-reranker-v2-m3 reranking (top_n=5)
     |
     v
  qwen2.5 (Ollama / llama.cpp) response generation
     |
     v
  Response + sources (path / page / heading path / score)
```

### 1.3 Component Responsibilities

| Layer | Technology | Location |
|-------|-----------|----------|
| Document conversion (PDF / image / Web URL) | **Docling Serve** | Docker, HTTP from Node (`/convert/file` or `/convert/source`) |
| Document conversion (SVG / drawio) | `fast-xml-parser` + `pako` | Node process |
| Document conversion (Markdown / text) | `node:fs` + `gray-matter` | Node process (no conversion needed, only frontmatter separation) |
| Chunking | `@langchain/textsplitters` + Japanese punctuation extension | Node process |
| Embedding | `bge-m3` on **Ollama** or **llama.cpp llama-server** | Docker / binary, HTTP from Node |
| Vector DB | **Qdrant** | Docker, HTTP from Node |
| Reranker | `@huggingface/transformers` (ONNX) | Node process |
| LLM | `qwen2.5` on **Ollama** or **llama.cpp** | Same |
| API | **Hono** | Node process, `127.0.0.1:7777` |
| CLI | `commander` | Node process, `pnpm rag` |
| External integration | curl / fetch / any HTTP client | No impact on existing assets |

---

## 2. Requirements

### 2.1 Hardware / Software

| Item | Requirement |
|------|-------------|
| OS | Linux (Ubuntu 22.04+ recommended) / macOS (Ollama path) |
| CPU | 8 cores or more |
| RAM | 32 GB (16 GB+ recommended for Ollama LLM inference) |
| GPU | Optional (NVIDIA + CUDA 12 for Ollama / llama.cpp acceleration) |
| Storage | SSD 100 GB+ (models + Qdrant data + Docling cache) |
| Node.js | **20.x+** (LTS recommended) |
| pnpm | **9.x+** (`npm install -g pnpm`) |
| Docker | 24.0+, Docker Compose v2.20+ |
| Ollama | 0.5.x+ (via Docker or host) |
| llama.cpp | `llama-server` binary (optional, alternative backend) |

### 2.2 Software Prerequisites Check

```bash
node --version            # v20.0.0+
pnpm --version            # 9.0+
docker --version
docker compose version
nvidia-smi                # When using GPU
```

### 2.3 Port Usage

| Service | Port | Config Variable |
|---------|------|-----------------|
| Qdrant REST | 6333 | `QDRANT_URL` |
| Qdrant gRPC | 6334 | - |
| Ollama | 11434 | `OLLAMA_HOST` |
| Docling Serve | 5001 | `DOCLING_URL` |
| llama.cpp llama-server (alternative) | 8080 (embeddings) / 8081 (LLM) | `LLAMACPP_EMBED_URL` / `LLAMACPP_LLM_URL` |
| **Hono API** (external client interface) | **7777** | `RAG_API_PORT` |

Override with environment variables if ports conflict. Verify:
```bash
ss -ltn '( sport = :7777 or sport = :6333 or sport = :11434 or sport = :5001 )'
```

---

## 3. Directory Layout

The RAG subsystem is contained within a **single project directory** `rag-system/`. The location is arbitrary -- it can be in `$HOME/projects/` or a subdirectory of an existing repository.

```
rag-system/                            <- RAG subsystem (place anywhere)
+-- design_rag.md                      <- This guide (place anywhere)
+-- pdf_image_rag_guide_nodejs.md      <- Reference guide (place anywhere, immutable)
+-- package.json
+-- pnpm-lock.yaml
+-- tsconfig.json
+-- docker-compose.yml
+-- .env.example
+-- .gitignore
+-- Makefile
+-- src/
|   +-- config.ts                      <- Environment variable loader
|   +-- logger.ts
|   +-- api/
|   |   +-- server.ts                  <- Hono server (port 7777)
|   +-- cli/
|   |   +-- index.ts                   <- `pnpm rag` entry point
|   +-- ingest/
|   |   +-- index.ts                   <- Ingestion dispatch (extension / URL detection)
|   |   +-- pdf.ts                     <- Docling Serve `/convert/file`
|   |   +-- svg.ts                     <- SVG XML parsing (new)
|   |   +-- drawio.ts                  <- drawio mxGraph parsing (new)
|   |   +-- markdown.ts                <- Markdown / text direct read (new)
|   |   +-- web.ts                     <- Web URL -> Docling Serve `/convert/source` (new)
|   +-- chunk/
|   |   +-- japanese.ts                <- Japanese punctuation-aware chunking
|   +-- embed/
|   |   +-- index.ts                   <- Backend switch
|   |   +-- ollama.ts
|   |   +-- llamacpp.ts                <- OpenAI-compatible /v1/embeddings
|   +-- search/
|   |   +-- qdrant.ts
|   |   +-- rerank.ts
|   +-- llm/
|   |   +-- index.ts
|   |   +-- ollama.ts
|   |   +-- llamacpp.ts                <- OpenAI-compatible /v1/chat/completions
|   +-- pipeline/
|       +-- ingest.ts                  <- Ingestion pipeline
|       +-- retrieve.ts                <- Search pipeline
+-- data/                              <- .gitignore (excluding binaries and models)
    +-- pdf/
    +-- svg/
    +-- drawio/
    +-- md/                            <- Direct ingestion Markdown
    +-- txt/                           <- Plain text
    +-- url/                           <- URL list (`*.urls` text format, one URL per line)
    +-- markdown/                      <- Conversion output (intermediate artifacts)
```

### 3.1 .gitignore (`rag/.gitignore`)

```gitignore
node_modules/
dist/
data/pdf/
data/svg/
data/drawio/
data/md/
data/txt/
data/url/
data/markdown/
.env
*.log
```

---

## 4. Phase 0: Project Initialization

### 4.1 Create Directories

```bash
mkdir -p rag-system/{src/{api,cli,ingest,chunk,embed,search,llm,pipeline},data/{pdf,svg,drawio,md,txt,url,markdown}}
cd rag-system
pnpm init
```

### 4.2 package.json

```json
{
  "name": "local-rag",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "engines": {
    "node": ">=20.0.0",
    "pnpm": ">=9.0.0"
  },
  "scripts": {
    "rag": "tsx src/cli/index.ts",
    "serve": "tsx src/api/server.ts",
    "build": "tsc -p tsconfig.json",
    "typecheck": "tsc --noEmit",
    "test": "vitest run"
  }
}
```

### 4.3 Dependencies

**Runtime**:

```bash
# HTTP / API
pnpm add hono @hono/node-server @hono/zod-validator

# Inference clients
pnpm add ollama
# llama.cpp path uses standard fetch + OpenAI-compatible, no additional dependencies

# Vector DB
pnpm add @qdrant/js-client-rest

# Reranker
pnpm add @huggingface/transformers

# Text processing
pnpm add @langchain/textsplitters gray-matter

# XML / compression (for SVG / drawio)
pnpm add fast-xml-parser pako

# Image (SVG OCR fallback, optional)
pnpm add sharp

# Utilities
pnpm add zod dotenv pino pino-pretty commander p-queue undici mime-types
```

**Dev dependencies**:

```bash
pnpm add -D typescript tsx vitest @types/node @types/mime-types @types/pako
```

### 4.4 tsconfig.json

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "lib": ["ES2023"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true,
    "sourceMap": true,
    "noUncheckedIndexedAccess": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

### 4.5 Environment Variables (`.env.example`)

```ini
# === Qdrant ===
QDRANT_URL=http://127.0.0.1:6333
QDRANT_API_KEY=
QDRANT_COLLECTION=rag_documents

# === Inference backend switch ===
RAG_BACKEND=ollama        # ollama | llamacpp

# === Ollama ===
OLLAMA_HOST=http://127.0.0.1:11434
OLLAMA_LLM_MODEL=qwen2.5:7b-instruct
OLLAMA_EMBED_MODEL=bge-m3

# === llama.cpp (alternative, OpenAI-compatible) ===
LLAMACPP_EMBED_URL=http://127.0.0.1:8080/v1
LLAMACPP_LLM_URL=http://127.0.0.1:8081/v1
LLAMACPP_EMBED_MODEL=bge-m3
LLAMACPP_LLM_MODEL=qwen2.5-7b-instruct

# === Docling Serve ===
DOCLING_URL=http://127.0.0.1:5001

# === Reranker ===
RERANKER_MODEL=onnx-community/bge-reranker-v2-m3-ONNX

# === Hono API ===
RAG_API_HOST=127.0.0.1
RAG_API_PORT=7777

# === Hyperparameters ===
CHUNK_SIZE=512
CHUNK_OVERLAP=64
TOP_K_RETRIEVE=20
TOP_K_RERANK=5
EMBED_DIM=1024
LOG_LEVEL=info
```

### 4.6 Configuration Loader (`src/config.ts`)

```ts
import 'dotenv/config';
import { z } from 'zod';

const envSchema = z.object({
  QDRANT_URL: z.string().url().default('http://127.0.0.1:6333'),
  QDRANT_API_KEY: z.string().optional(),
  QDRANT_COLLECTION: z.string().default('rag_documents'),

  RAG_BACKEND: z.enum(['ollama', 'llamacpp']).default('ollama'),

  OLLAMA_HOST: z.string().url().default('http://127.0.0.1:11434'),
  OLLAMA_LLM_MODEL: z.string().default('qwen2.5:7b-instruct'),
  OLLAMA_EMBED_MODEL: z.string().default('bge-m3'),

  LLAMACPP_EMBED_URL: z.string().url().default('http://127.0.0.1:8080/v1'),
  LLAMACPP_LLM_URL: z.string().url().default('http://127.0.0.1:8081/v1'),
  LLAMACPP_EMBED_MODEL: z.string().default('bge-m3'),
  LLAMACPP_LLM_MODEL: z.string().default('qwen2.5-7b-instruct'),

  DOCLING_URL: z.string().url().default('http://127.0.0.1:5001'),
  RERANKER_MODEL: z.string().default('onnx-community/bge-reranker-v2-m3-ONNX'),

  RAG_API_HOST: z.string().default('127.0.0.1'),
  RAG_API_PORT: z.coerce.number().default(7777),

  CHUNK_SIZE: z.coerce.number().default(512),
  CHUNK_OVERLAP: z.coerce.number().default(64),
  TOP_K_RETRIEVE: z.coerce.number().default(20),
  TOP_K_RERANK: z.coerce.number().default(5),
  EMBED_DIM: z.coerce.number().default(1024),

  LOG_LEVEL: z.enum(['trace', 'debug', 'info', 'warn', 'error']).default('info'),
});

export const config = envSchema.parse(process.env);
export type Config = typeof config;
```

### 4.7 Logger (`src/logger.ts`)

```ts
import pino from 'pino';
import { config } from './config.js';

export const logger = pino({
  level: config.LOG_LEVEL,
  transport: process.env.NODE_ENV !== 'production'
    ? { target: 'pino-pretty', options: { colorize: true, translateTime: 'HH:MM:ss' } }
    : undefined,
});
```

### 4.8 Verification

```bash
cd rag-system
pnpm typecheck
# Expected: no errors (even if src/ is empty, as long as tsconfig is valid)
```

---

## 5. Phase 1: Docker Compose

### 5.1 `rag/docker-compose.yml`

```yaml
services:
  qdrant:
    image: qdrant/qdrant:v1.12.4
    container_name: rag-qdrant
    restart: unless-stopped
    ports:
      - "127.0.0.1:6333:6333"
      - "127.0.0.1:6334:6334"
    volumes:
      - qdrant_data:/qdrant/storage
    environment:
      QDRANT__SERVICE__GRPC_PORT: 6334
      QDRANT__LOG_LEVEL: INFO

  ollama:
    image: ollama/ollama:0.5.4
    container_name: rag-ollama
    restart: unless-stopped
    ports:
      - "127.0.0.1:11434:11434"
    volumes:
      - ollama_data:/root/.ollama
    # Remove deploy section if no GPU is available
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]

  docling:
    image: quay.io/docling-project/docling-serve-cpu:latest
    # GPU version: quay.io/docling-project/docling-serve-cu128:latest
    container_name: rag-docling
    restart: unless-stopped
    ports:
      - "127.0.0.1:5001:5001"
    environment:
      DOCLING_SERVE_ENABLE_UI: "1"
      DOCLING_SERVE_API_HOST: "0.0.0.0"
      DOCLING_SERVE_API_PORT: "5001"
    volumes:
      - docling_cache:/opt/app-root/src/.cache

volumes:
  qdrant_data:
  ollama_data:
  docling_cache:
```

### 5.2 Start and Pull Models

```bash
cd rag-system
docker compose up -d
docker compose ps

# Verify services
curl -fsS http://127.0.0.1:6333/readyz       # Qdrant
curl -fsS http://127.0.0.1:11434/api/tags    # Ollama
curl -fsS http://127.0.0.1:5001/health       # Docling Serve

# Pull models into Ollama
docker exec rag-ollama ollama pull qwen2.5:7b-instruct
docker exec rag-ollama ollama pull bge-m3
docker exec rag-ollama ollama list
```

### 5.3 Verification

```bash
curl -fsS http://127.0.0.1:6333/readyz
# Expected: 200 OK + status JSON

curl -fsS -X POST http://127.0.0.1:11434/api/embed \
  -d '{"model":"bge-m3","input":"test text"}' | jq '.embeddings[0] | length'
# Expected: 1024
```

### 5.4 llama.cpp Alternative (Optional)

Start `llama-server` on two ports (requires pre-downloaded GGUF models):

```bash
# Embeddings (port 8080)
llama-server -m models/bge-m3-Q5_K_M.gguf --port 8080 --embeddings &

# LLM (port 8081)
llama-server -m models/qwen2.5-7b-instruct-Q5_K_M.gguf --port 8081 -c 8192 &
```

Both provide OpenAI-compatible APIs (`/v1/embeddings`, `/v1/chat/completions`), so switching `.env` to `RAG_BACKEND=llamacpp` is all that's needed.

---

## 6. Phase 2: Document Conversion (PDF / Image / SVG / drawio / Markdown / Text / Web)

### 6.1 Conversion Dispatch (`src/ingest/index.ts`)

Input accepts **local file paths** or **URLs (`http://` / `https://`)**. Files with the `.urls` extension are treated as URL lists, expanded line by line.

```ts
import { extname } from 'node:path';
import { convertPdf } from './pdf.js';
import { convertSvg } from './svg.js';
import { convertDrawio } from './drawio.js';
import { convertMarkdown, convertText } from './markdown.js';
import { convertWeb } from './web.js';

export interface ConvertedDoc {
  source: string;
  markdown: string;
  metadata: Record<string, unknown>;
}

export function isUrl(input: string): boolean {
  return /^https?:\/\//i.test(input);
}

export async function convertAny(input: string): Promise<ConvertedDoc> {
  if (isUrl(input)) return convertWeb(input);
  const ext = extname(input).toLowerCase();
  if (ext === '.pdf' || ['.png', '.jpg', '.jpeg', '.tiff', '.bmp'].includes(ext)) {
    return convertPdf(input);
  }
  if (ext === '.svg' || input.endsWith('.drawio.svg')) {
    return convertSvg(input);
  }
  if (ext === '.drawio') {
    return convertDrawio(input);
  }
  if (ext === '.md' || ext === '.markdown') {
    return convertMarkdown(input);
  }
  if (ext === '.txt' || ext === '.log' || ext === '.rst') {
    return convertText(input);
  }
  throw new Error(`Unsupported format: ${input}`);
}
```

### 6.2 PDF / Image (Docling Serve, inherited from reference guide)

`src/ingest/pdf.ts`:

```ts
import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { config } from '../config.js';
import { logger } from '../logger.js';
import type { ConvertedDoc } from './index.js';

export async function convertPdf(filePath: string): Promise<ConvertedDoc> {
  const data = await readFile(filePath);
  const blob = new Blob([data]);
  const form = new FormData();
  form.append('files', blob, basename(filePath));
  form.append('parameters', JSON.stringify({
    from_formats: ['pdf', 'image', 'docx', 'pptx', 'html', 'md'],
    to_formats: ['md'],
    do_ocr: true,
    ocr_engine: 'easyocr',
    ocr_lang: ['ja', 'en'],
    table_mode: 'accurate',
    image_export_mode: 'placeholder',
    abort_on_error: false,
    return_as_file: false,
  }));

  const url = `${config.DOCLING_URL}/v1alpha/convert/file`;
  logger.info({ filePath }, 'Calling Docling Serve');
  const res = await fetch(url, { method: 'POST', body: form, signal: AbortSignal.timeout(10 * 60 * 1000) });
  if (!res.ok) throw new Error(`Docling failed (${res.status}): ${await res.text()}`);

  const json = await res.json() as { document?: { md_content?: string }, status?: string, errors?: Array<{ error_message: string }> };
  if (json.status !== 'success' && json.errors?.length) {
    throw new Error(`Docling errors: ${json.errors.map(e => e.error_message).join('; ')}`);
  }
  const md = json.document?.md_content ?? '';
  if (!md) throw new Error('Empty markdown from Docling');
  return { source: filePath, markdown: md, metadata: { kind: 'pdf' } };
}
```

### 6.3 SVG Ingestion (**New**)

`src/ingest/svg.ts`:

```ts
import { readFile } from 'node:fs/promises';
import { XMLParser } from 'fast-xml-parser';
import { basename } from 'node:path';
import type { ConvertedDoc } from './index.js';

interface SvgTextElement {
  text: string;
  x?: number;
  y?: number;
  kind: 'text' | 'tspan' | 'title' | 'desc';
}

const TEXT_TAGS = new Set(['text', 'tspan', 'title', 'desc']);

function walk(node: unknown, acc: SvgTextElement[]): void {
  if (node == null || typeof node !== 'object') return;
  for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
    if (Array.isArray(value)) {
      for (const v of value) walkTagged(key, v, acc);
    } else if (typeof value === 'object') {
      walkTagged(key, value, acc);
    }
  }
}

function walkTagged(tag: string, node: unknown, acc: SvgTextElement[]): void {
  if (node == null || typeof node !== 'object') return;
  const obj = node as Record<string, unknown>;
  if (TEXT_TAGS.has(tag)) {
    const text = collectText(obj);
    if (text.trim()) {
      acc.push({
        kind: tag as SvgTextElement['kind'],
        x: typeof obj.x === 'string' ? Number(obj.x) : (obj.x as number | undefined),
        y: typeof obj.y === 'string' ? Number(obj.y) : (obj.y as number | undefined),
        text: text.trim(),
      });
    }
  }
  walk(obj, acc);
}

function collectText(obj: Record<string, unknown>): string {
  if (typeof obj['#text'] === 'string') return obj['#text'];
  let s = '';
  for (const [k, v] of Object.entries(obj)) {
    if (k === '#text' && typeof v === 'string') s += v;
    if (Array.isArray(v)) for (const vi of v) if (vi && typeof vi === 'object') s += collectText(vi as Record<string, unknown>);
    if (v && typeof v === 'object' && !Array.isArray(v)) s += collectText(v as Record<string, unknown>);
  }
  return s;
}

export async function convertSvg(filePath: string): Promise<ConvertedDoc> {
  const xml = await readFile(filePath, 'utf-8');
  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: '',
    preserveOrder: false,
    allowBooleanAttributes: true,
  });
  const root = parser.parse(xml);
  const acc: SvgTextElement[] = [];
  walk(root, acc);

  // .drawio.svg may have mxGraph XML embedded in <svg content="...">
  // -> Try to extract using drawio.ts's extractMxCells for shared handling
  const svgRoot = (root as Record<string, unknown>).svg as Record<string, unknown> | undefined;
  const embeddedContent = svgRoot?.content;
  let drawioMd = '';
  if (typeof embeddedContent === 'string' && embeddedContent.trim().startsWith('<')) {
    const { extractMxCells } = await import('./drawio.js');
    drawioMd = extractMxCells(embeddedContent);
  }

  const lines: string[] = [`# SVG: ${basename(filePath)}`, ''];
  for (const el of acc) {
    const pos = el.x != null && el.y != null ? ` (x=${el.x}, y=${el.y})` : '';
    lines.push(`- [${el.kind}]${pos} ${el.text}`);
  }
  if (drawioMd) {
    lines.push('', '## Embedded drawio cells', '', drawioMd);
  }
  return {
    source: filePath,
    markdown: lines.join('\n'),
    metadata: { kind: 'svg', elementCount: acc.length },
  };
}
```

### 6.4 drawio Ingestion (**New**)

`src/ingest/drawio.ts`:

```ts
import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { XMLParser } from 'fast-xml-parser';
import pako from 'pako';
import type { ConvertedDoc } from './index.js';

/**
 * The content of a drawio <diagram> element is one of:
 *   1) Plain text mxGraph XML (<mxGraphModel>...</mxGraphModel>)
 *   2) Deflate + base64 encoded URL-encoded mxGraph XML
 */
export function decompressDiagram(content: string): string {
  const trimmed = content.trim();
  if (trimmed.startsWith('<mxGraphModel') || trimmed.startsWith('<?xml')) {
    return trimmed;
  }
  // base64 -> bytes -> inflateRaw -> URL decode
  const buf = Buffer.from(trimmed, 'base64');
  const inflated = pako.inflateRaw(buf, { to: 'string' });
  return decodeURIComponent(inflated);
}

interface MxCellLabel { id?: string; label: string; parent?: string; }

export function extractMxCells(mxXml: string): string {
  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: '',
    allowBooleanAttributes: true,
  });
  const root = parser.parse(mxXml);
  const cells: MxCellLabel[] = [];
  collectCells(root, cells);
  return cells.map(c => `- ${c.label}${c.id ? ` (id=${c.id})` : ''}`).join('\n');
}

function collectCells(node: unknown, acc: MxCellLabel[]): void {
  if (!node || typeof node !== 'object') return;
  const obj = node as Record<string, unknown>;
  for (const [key, value] of Object.entries(obj)) {
    const items = Array.isArray(value) ? value : [value];
    for (const item of items) {
      if (!item || typeof item !== 'object') continue;
      const it = item as Record<string, unknown>;
      if (key === 'mxCell' || key === 'UserObject') {
        const label = (it.value ?? it.label) as string | undefined;
        if (typeof label === 'string' && label.trim()) {
          acc.push({
            id: typeof it.id === 'string' ? it.id : undefined,
            label: stripHtml(label).trim(),
          });
        }
      }
      collectCells(it, acc);
    }
  }
}

function stripHtml(s: string): string {
  return s.replace(/<[^>]+>/g, ' ').replace(/&nbsp;/g, ' ').replace(/\s+/g, ' ');
}

export async function convertDrawio(filePath: string): Promise<ConvertedDoc> {
  const xml = await readFile(filePath, 'utf-8');
  const parser = new XMLParser({ ignoreAttributes: false, attributeNamePrefix: '' });
  const root = parser.parse(xml);

  // Multi-level nesting: mxfile > diagram[*] > (compressed | mxGraphModel)
  const diagrams = collectDiagrams(root);
  const lines: string[] = [`# drawio: ${basename(filePath)}`, ''];
  for (const [i, d] of diagrams.entries()) {
    const inner = decompressDiagram(typeof d === 'string' ? d : (d as { '#text'?: string })['#text'] ?? '');
    lines.push(`## Diagram ${i + 1}${typeof d === 'object' && (d as Record<string, unknown>).name ? `: ${(d as Record<string, unknown>).name}` : ''}`, '', extractMxCells(inner), '');
  }
  return {
    source: filePath,
    markdown: lines.join('\n'),
    metadata: { kind: 'drawio', diagramCount: diagrams.length },
  };
}

function collectDiagrams(node: unknown): unknown[] {
  if (!node || typeof node !== 'object') return [];
  const obj = node as Record<string, unknown>;
  if (Array.isArray(obj.diagram)) return obj.diagram;
  if (obj.diagram) return [obj.diagram];
  for (const v of Object.values(obj)) {
    if (v && typeof v === 'object') {
      const r = collectDiagrams(v);
      if (r.length) return r;
    }
  }
  return [];
}
```

### 6.5 Markdown / Plain Text Ingestion (**New**)

`src/ingest/markdown.ts`:

```ts
import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import matter from 'gray-matter';
import type { ConvertedDoc } from './index.js';

/**
 * Markdown is ingested as-is. Frontmatter is separated into metadata.
 */
export async function convertMarkdown(filePath: string): Promise<ConvertedDoc> {
  const raw = await readFile(filePath, 'utf-8');
  const { content, data: frontmatter } = matter(raw);
  return {
    source: filePath,
    markdown: content,
    metadata: {
      kind: 'markdown',
      frontmatter: Object.keys(frontmatter).length ? frontmatter : undefined,
      title: typeof frontmatter.title === 'string' ? frontmatter.title : basename(filePath),
    },
  };
}

/**
 * Plain text / .log / .rst receives minimal formatting and is treated like Markdown.
 * - BOM removal
 * - CRLF -> LF
 * - Compress consecutive blank lines (prevents excessive chunking)
 */
export async function convertText(filePath: string): Promise<ConvertedDoc> {
  let raw = await readFile(filePath, 'utf-8');
  if (raw.charCodeAt(0) === 0xfeff) raw = raw.slice(1);
  raw = raw.replace(/\r\n/g, '\n').replace(/\n{3,}/g, '\n\n');
  return {
    source: filePath,
    markdown: `# ${basename(filePath)}\n\n${raw}`,
    metadata: { kind: 'text' },
  };
}
```

### 6.6 Web (URL) Ingestion (**New**)

Docling Serve provides a `/v1alpha/convert/source` endpoint that directly accepts URLs. It fetches, parses, and converts HTML/PDF/images on the server side. No need to build a fetch + cheerio + html-to-md pipeline in Node.

`src/ingest/web.ts`:

```ts
import { config } from '../config.js';
import { logger } from '../logger.js';
import type { ConvertedDoc } from './index.js';

interface DoclingSourceResponse {
  document?: { md_content?: string; filename?: string };
  status?: string;
  errors?: Array<{ error_message: string }>;
}

/**
 * Pass a URL to Docling Serve to get Markdown.
 * - Only HTTP/HTTPS URLs are allowed
 * - HTML/PDF/images are all handled through a single path
 * - OCR does not trigger for HTML but does for PDF/image URLs automatically
 */
export async function convertWeb(url: string): Promise<ConvertedDoc> {
  if (!/^https?:\/\//i.test(url)) throw new Error(`Not an HTTP(S) URL: ${url}`);

  const body = {
    sources: [{ kind: 'http', url }],
    options: {
      from_formats: ['pdf', 'image', 'docx', 'pptx', 'html', 'md'],
      to_formats: ['md'],
      do_ocr: true,
      ocr_engine: 'easyocr',
      ocr_lang: ['ja', 'en'],
      table_mode: 'accurate',
      image_export_mode: 'placeholder',
      abort_on_error: false,
      return_as_file: false,
    },
  };

  logger.info({ url }, 'Calling Docling Serve (URL source)');
  const res = await fetch(`${config.DOCLING_URL}/v1alpha/convert/source`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(10 * 60 * 1000),
  });
  if (!res.ok) throw new Error(`Docling URL convert failed (${res.status}): ${await res.text()}`);

  const json = await res.json() as DoclingSourceResponse;
  if (json.status !== 'success' && json.errors?.length) {
    throw new Error(`Docling errors: ${json.errors.map(e => e.error_message).join('; ')}`);
  }
  const md = json.document?.md_content ?? '';
  if (!md) throw new Error(`Empty markdown from URL: ${url}`);

  return {
    source: url,
    markdown: md,
    metadata: {
      kind: 'web',
      url,
      filename: json.document?.filename,
      fetchedAt: new Date().toISOString(),
    },
  };
}

/**
 * Reads a `.urls` text file (one URL per line, `#` lines are comments) and returns a URL array.
 * Expanded by `expandPath`, each URL is ingested individually via `convertWeb`.
 */
export async function readUrlList(filePath: string): Promise<string[]> {
  const { readFile } = await import('node:fs/promises');
  const raw = await readFile(filePath, 'utf-8');
  return raw.split(/\r?\n/).map(l => l.trim()).filter(l => l && !l.startsWith('#'));
}
```

#### Web Ingestion Notes

- **Scope**: A single URL is ingested as one document. Site-wide crawling (recursive N-depth traversal) is out of scope. Use `wget --mirror` or similar to download locally before ingesting.
- **Authentication**: Authenticated URLs cannot be accessed directly by Docling Serve. If Cookie headers are needed, download via `wget` first, then ingest the local file.
- **Bulk URLs**: A `.urls` file (one URL per line) can be bulk-ingested via `pnpm rag ingest path/to/list.urls` (see Phase 9).
- **Updates**: Re-ingesting the same URL adds new chunks (duplicates). Before re-ingesting, use `pnpm rag reindex` to clear the collection, or implement a deduplication strategy using `payload.url` as a key.

### 6.7 Verification

```bash
# SVG: <text>Hello</text> sample
cat > rag-system/data/svg/hello.svg <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg"><text x="10" y="20">Hello World</text></svg>
EOF
pnpm tsx -e "import('./src/ingest/svg.js').then(m => m.convertSvg('data/svg/hello.svg').then(r => console.log(r.markdown)))"
# Expected: Markdown containing "- [text] (x=10, y=20) Hello World"

# drawio: place a compressed drawio file exported from drawio Web
ls rag-system/data/drawio/*.drawio
pnpm tsx -e "import('./src/ingest/drawio.js').then(m => m.convertDrawio('data/drawio/sample.drawio').then(r => console.log(r.markdown.slice(0, 500))))"
# Expected: "# drawio: sample.drawio" + "## Diagram 1" + label list

# Markdown: direct read
cat > rag-system/data/md/note.md <<'EOF'
---
title: Note
---
# Summary

This is a sample memo document.
EOF
pnpm tsx -e "import('./src/ingest/markdown.js').then(m => m.convertMarkdown('data/md/note.md').then(r => console.log(JSON.stringify(r, null, 2))))"
# Expected: metadata.frontmatter.title === 'Note', markdown body contains "# Summary"

# Plain text
cat > rag-system/data/txt/changelog.txt <<'EOF'
2026-05-06: Added ingestion pipeline.
2026-05-07: Added web ingestion support.
EOF
pnpm tsx -e "import('./src/ingest/markdown.js').then(m => m.convertText('data/txt/changelog.txt').then(r => console.log(r.markdown)))"
# Expected: "# changelog.txt" + 2 lines of body text

# Web URL (requires Docling Serve running)
pnpm tsx -e "import('./src/ingest/web.js').then(m => m.convertWeb('https://example.com').then(r => console.log(r.markdown.slice(0, 200))))"
# Expected: "Example Domain" etc. from example.com as Markdown

# URL list
cat > rag-system/data/url/refs.urls <<'EOF'
# Reference links (comment lines are ignored)
https://example.com
https://qdrant.tech/documentation/
EOF
pnpm tsx -e "import('./src/ingest/web.js').then(async m => console.log(await m.readUrlList('data/url/refs.urls')))"
# Expected: ['https://example.com', 'https://qdrant.tech/documentation/']
```

---

## 7. Phase 3: Chunking (Japanese-Enhanced)

### 7.1 `src/chunk/japanese.ts`

```ts
import {
  MarkdownTextSplitter,
  RecursiveCharacterTextSplitter,
} from '@langchain/textsplitters';
import matter from 'gray-matter';
import { config } from '../config.js';

export interface Chunk {
  text: string;
  metadata: {
    source: string;
    chunkId: number;
    headings: string[];
    frontmatter?: Record<string, unknown>;
  };
}

const JP_SEPARATORS = [
  '\n\n',  // Paragraph
  '\n',    // Line
  '。', '！', '？',  // Japanese sentence-ending punctuation
  '. ', '! ', '? ', // English sentence endings
  '、',                // Japanese comma (last resort)
  ' ',                 // Space
  '',                  // Character level
];

export async function chunkJapanese(
  source: string,
  markdown: string,
): Promise<Chunk[]> {
  const { content, data: frontmatter } = matter(markdown);

  // Coarse split by heading structure
  const mdSplitter = new MarkdownTextSplitter({
    chunkSize: config.CHUNK_SIZE * 4,
    chunkOverlap: 0,
  });
  const sections = await mdSplitter.splitText(content);

  // Fine-grained split with Japanese punctuation priority
  const refine = new RecursiveCharacterTextSplitter({
    chunkSize: config.CHUNK_SIZE * 3,        // 1 token ~= 2-3 characters (bge-m3 Japanese)
    chunkOverlap: config.CHUNK_OVERLAP * 3,
    separators: JP_SEPARATORS,
  });

  const chunks: Chunk[] = [];
  let idx = 0;

  for (const section of sections) {
    const headings = extractHeadings(section);
    const parts = await refine.splitText(section);
    for (const body of parts) {
      const text = contextualize(headings, body);
      if (text.trim().length < 8) continue; // Skip very small chunks
      chunks.push({
        text,
        metadata: {
          source,
          chunkId: idx++,
          headings,
          frontmatter: Object.keys(frontmatter).length ? frontmatter : undefined,
        },
      });
    }
  }
  return chunks;
}

function extractHeadings(md: string): string[] {
  const out: string[] = [];
  for (const line of md.split('\n')) {
    const m = /^(#{1,6})\s+(.+?)\s*$/.exec(line);
    if (m) out.push(m[2]);
  }
  return out;
}

function contextualize(headings: string[], body: string): string {
  if (headings.length === 0) return body;
  return `${headings.map(h => `# ${h}`).join(' > ')}\n\n${body}`;
}
```

### 7.2 Verification

```bash
pnpm tsx -e "
import('./src/chunk/japanese.js').then(async m => {
  const chunks = await m.chunkJapanese('test.md', '# Summary\n\nThis system is a RAG pipeline. It ingests Japanese documents. Search and reranking return top results.');
  console.log('chunks:', chunks.length);
  for (const c of chunks) console.log('-', c.text.slice(0, 80));
})"
# Expected: 1-3 chunks, body starts with "# Summary" heading prefix
```

---

## 8. Phase 4: Embedding Generation (Ollama Primary / llama.cpp Alternative)

### 8.1 Dispatch (`src/embed/index.ts`)

```ts
import { config } from '../config.js';
import { embedOllama } from './ollama.js';
import { embedLlamaCpp } from './llamacpp.js';

export async function embed(texts: string[]): Promise<number[][]> {
  return config.RAG_BACKEND === 'llamacpp' ? embedLlamaCpp(texts) : embedOllama(texts);
}

export async function embedOne(text: string): Promise<number[]> {
  const [v] = await embed([text]);
  if (!v) throw new Error('Empty embedding');
  return v;
}
```

### 8.2 Ollama Path (`src/embed/ollama.ts`)

```ts
import { Ollama } from 'ollama';
import { config } from '../config.js';
import { logger } from '../logger.js';

const client = new Ollama({ host: config.OLLAMA_HOST });

export async function embedOllama(texts: string[], batchSize = 16): Promise<number[][]> {
  const result: number[][] = [];
  for (let i = 0; i < texts.length; i += batchSize) {
    const batch = texts.slice(i, i + batchSize);
    const res = await client.embed({
      model: config.OLLAMA_EMBED_MODEL,
      input: batch,
      truncate: true,
    });
    if (!res.embeddings || res.embeddings.length !== batch.length) throw new Error('Unexpected response');
    for (const v of res.embeddings) {
      if (v.length !== config.EMBED_DIM) throw new Error(`Dim mismatch: ${v.length}`);
      result.push(v);
    }
    logger.debug({ done: result.length, total: texts.length }, 'embed (ollama)');
  }
  return result;
}
```

### 8.3 llama.cpp Path (`src/embed/llamacpp.ts`, OpenAI-compatible)

```ts
import { config } from '../config.js';
import { logger } from '../logger.js';

interface OAEmbedResponse {
  data: Array<{ embedding: number[]; index: number }>;
}

export async function embedLlamaCpp(texts: string[], batchSize = 8): Promise<number[][]> {
  const result: number[][] = [];
  for (let i = 0; i < texts.length; i += batchSize) {
    const batch = texts.slice(i, i + batchSize);
    const res = await fetch(`${config.LLAMACPP_EMBED_URL}/embeddings`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ model: config.LLAMACPP_EMBED_MODEL, input: batch }),
    });
    if (!res.ok) throw new Error(`llamacpp embed failed (${res.status}): ${await res.text()}`);
    const json = await res.json() as OAEmbedResponse;
    json.data.sort((a, b) => a.index - b.index);
    for (const d of json.data) {
      if (d.embedding.length !== config.EMBED_DIM) throw new Error(`Dim mismatch: ${d.embedding.length}`);
      result.push(d.embedding);
    }
    logger.debug({ done: result.length, total: texts.length }, 'embed (llamacpp)');
  }
  return result;
}
```

### 8.4 Verification

```bash
RAG_BACKEND=ollama pnpm tsx -e "
import('./src/embed/index.js').then(async m => {
  const v = await m.embedOne('What are the components of a RAG pipeline?');
  console.log('dim:', v.length);
})"
# Expected: dim: 1024

# llama.cpp path (only when llama-server is running)
RAG_BACKEND=llamacpp pnpm tsx -e "
import('./src/embed/index.js').then(async m => {
  const v = await m.embedOne('test');
  console.log('dim:', v.length);
})"
```

---

## 9. Phase 5: Qdrant Ingestion

### 9.1 `src/search/qdrant.ts`

```ts
import { QdrantClient } from '@qdrant/js-client-rest';
import { config } from '../config.js';
import { logger } from '../logger.js';

export const DENSE = 'dense';

export function getQdrantClient(): QdrantClient {
  return new QdrantClient({
    url: config.QDRANT_URL,
    apiKey: config.QDRANT_API_KEY || undefined,
    checkCompatibility: false,
  });
}

export async function ensureCollection(client: QdrantClient, recreate = false): Promise<void> {
  const list = await client.getCollections();
  const exists = list.collections.some(c => c.name === config.QDRANT_COLLECTION);
  if (exists && recreate) await client.deleteCollection(config.QDRANT_COLLECTION);
  if (!exists || recreate) {
    await client.createCollection(config.QDRANT_COLLECTION, {
      vectors: { [DENSE]: { size: config.EMBED_DIM, distance: 'Cosine', on_disk: false } },
      hnsw_config: { m: 16, ef_construct: 128 },
      optimizers_config: { indexing_threshold: 20_000 },
    });
    await client.createPayloadIndex(config.QDRANT_COLLECTION, { field_name: 'source', field_schema: 'keyword' });
    await client.createPayloadIndex(config.QDRANT_COLLECTION, { field_name: 'kind', field_schema: 'keyword' });
    await client.createPayloadIndex(config.QDRANT_COLLECTION, { field_name: 'url', field_schema: 'keyword' });
    await client.createPayloadIndex(config.QDRANT_COLLECTION, { field_name: 'chunk_id', field_schema: 'integer' });
    logger.info({ collection: config.QDRANT_COLLECTION }, 'Collection created');
  }
}

export interface UpsertPoint {
  id: string;
  vector: number[];
  payload: Record<string, unknown>;
}

export async function upsertPoints(client: QdrantClient, points: UpsertPoint[]): Promise<void> {
  if (!points.length) return;
  await client.upsert(config.QDRANT_COLLECTION, {
    wait: false,
    points: points.map(p => ({
      id: p.id,
      vector: { [DENSE]: p.vector },
      payload: p.payload,
    })),
  });
}

export async function denseSearch(
  client: QdrantClient,
  vector: number[],
  limit: number,
  filter?: Record<string, unknown>,
): Promise<Array<{ id: string | number; score: number; payload: Record<string, unknown> }>> {
  const res = await client.query(config.QDRANT_COLLECTION, {
    query: vector,
    using: DENSE,
    limit,
    with_payload: true,
    filter: filter as never,
  });
  return res.points.map(p => ({ id: p.id, score: p.score ?? 0, payload: p.payload ?? {} }));
}
```

### 9.2 Ingestion Pipeline (`src/pipeline/ingest.ts`)

```ts
import { randomUUID } from 'node:crypto';
import PQueue from 'p-queue';
import { glob } from 'node:fs/promises';
import { join } from 'node:path';
import { config } from '../config.js';
import { logger } from '../logger.js';
import { convertAny } from '../ingest/index.js';
import { chunkJapanese } from '../chunk/japanese.js';
import { embed } from '../embed/index.js';
import { ensureCollection, getQdrantClient, upsertPoints } from '../search/qdrant.js';

export interface IngestStats { ingested: number; chunks: number; errors: number; }

export async function ingestPath(input: string): Promise<{ chunks: number }> {
  const doc = await convertAny(input);
  const chunks = await chunkJapanese(doc.source, doc.markdown);
  if (!chunks.length) return { chunks: 0 };

  const client = getQdrantClient();
  await ensureCollection(client);

  const BATCH = 32;
  let total = 0;
  for (let i = 0; i < chunks.length; i += BATCH) {
    const slice = chunks.slice(i, i + BATCH);
    const vectors = await embed(slice.map(c => c.text));
    const points = slice.map((c, j) => ({
      id: randomUUID(),
      vector: vectors[j]!,
      payload: {
        text: c.text,
        source: c.metadata.source,
        chunk_id: c.metadata.chunkId,
        headings: c.metadata.headings,
        kind: doc.metadata.kind,
        url: doc.metadata.kind === 'web' ? doc.metadata.url : undefined,
      },
    }));
    await upsertPoints(client, points);
    total += points.length;
  }
  logger.info({ input, chunks: total }, 'Ingested');
  return { chunks: total };
}

export async function ingestPaths(inputs: string[]): Promise<IngestStats> {
  const stats: IngestStats = { ingested: 0, chunks: 0, errors: 0 };
  const queue = new PQueue({ concurrency: 2 });
  await Promise.all(inputs.map(p => queue.add(async () => {
    try {
      const r = await ingestPath(p);
      stats.ingested++;
      stats.chunks += r.chunks;
    } catch (e) {
      stats.errors++;
      logger.error({ input: p, err: (e as Error).message }, 'Ingest failed');
    }
  })));
  return stats;
}

/**
 * Expand a single input into ingestion targets:
 *   - URL (http(s)://...)          -> single item
 *   - .urls file                   -> expanded to one URL per line
 *   - Single file                  -> single item
 *   - Directory                   -> recursively enumerate matching extensions
 */
export async function expandPath(target: string): Promise<string[]> {
  if (/^https?:\/\//i.test(target)) return [target];

  const { stat } = await import('node:fs/promises');
  const st = await stat(target);
  if (st.isFile()) {
    if (target.endsWith('.urls')) {
      const { readUrlList } = await import('../ingest/web.js');
      return readUrlList(target);
    }
    return [target];
  }

  const patterns = [
    '**/*.pdf', '**/*.png', '**/*.jpg', '**/*.jpeg', '**/*.tiff', '**/*.bmp',
    '**/*.svg', '**/*.drawio', '**/*.drawio.svg',
    '**/*.md', '**/*.markdown', '**/*.txt', '**/*.log', '**/*.rst',
    '**/*.urls',
  ];
  const out: string[] = [];
  for (const pat of patterns) {
    for await (const f of glob(pat, { cwd: target })) {
      const full = join(target, f);
      if (full.endsWith('.urls')) {
        const { readUrlList } = await import('../ingest/web.js');
        out.push(...await readUrlList(full));
      } else {
        out.push(full);
      }
    }
  }
  return out;
}
```

### 9.3 Verification

```bash
curl -fsS http://127.0.0.1:6333/collections | jq
# Expected: before ingestion, empty; after ingestion, { "collections": [{"name":"rag_documents"}] }

# Place a simple PDF / SVG in data/ and ingest
pnpm rag ingest data/svg/hello.svg
curl -fsS http://127.0.0.1:6333/collections/rag_documents | jq '.result.points_count'
# Expected: 1 or more
```

---

## 10. Phase 6: Search + Reranking

### 10.1 Reranker (`src/search/rerank.ts`)

```ts
import {
  AutoModelForSequenceClassification,
  AutoTokenizer,
  type PreTrainedTokenizer,
  type PreTrainedModel,
} from '@huggingface/transformers';
import { config } from '../config.js';
import { logger } from '../logger.js';

let cached: { tokenizer: PreTrainedTokenizer; model: PreTrainedModel } | null = null;

async function getReranker() {
  if (cached) return cached;
  logger.info({ model: config.RERANKER_MODEL }, 'Loading reranker (first time is slow)');
  const tokenizer = await AutoTokenizer.from_pretrained(config.RERANKER_MODEL);
  const model = await AutoModelForSequenceClassification.from_pretrained(config.RERANKER_MODEL, { dtype: 'fp32' });
  cached = { tokenizer, model };
  return cached;
}

export async function rerank(
  query: string,
  passages: string[],
  topN: number = config.TOP_K_RERANK,
): Promise<Array<{ index: number; text: string; score: number }>> {
  if (!passages.length) return [];
  const { tokenizer, model } = await getReranker();
  const queries = passages.map(() => query);
  const inputs = tokenizer(queries, { text_pair: passages, padding: true, truncation: true, max_length: 512 });
  const out = await (model as unknown as (i: unknown) => Promise<{ logits: { data: Float32Array } }>)(inputs);
  const scores = Array.from(out.logits.data);
  return passages
    .map((text, i) => ({ index: i, text, score: scores[i] ?? -Infinity }))
    .sort((a, b) => b.score - a.score)
    .slice(0, topN);
}
```

### 10.2 Search Pipeline (`src/pipeline/retrieve.ts`)

```ts
import { config } from '../config.js';
import { embedOne } from '../embed/index.js';
import { denseSearch, getQdrantClient } from '../search/qdrant.js';
import { rerank } from '../search/rerank.js';

export interface RetrievedDoc {
  id: string | number;
  text: string;
  score: number;
  rerankScore?: number;
  source: string;
  chunkId: number;
  headings: string[];
  kind?: string;
}

export async function retrieve(
  query: string,
  opts: { topK?: number; topN?: number; rerank?: boolean; filter?: Record<string, unknown> } = {},
): Promise<RetrievedDoc[]> {
  const topK = opts.topK ?? config.TOP_K_RETRIEVE;
  const topN = opts.topN ?? config.TOP_K_RERANK;

  const client = getQdrantClient();
  const qvec = await embedOne(query);
  const hits = await denseSearch(client, qvec, topK, opts.filter);

  const candidates = hits.map(h => ({
    id: h.id,
    text: (h.payload['text'] as string) ?? '',
    score: h.score,
    source: (h.payload['source'] as string) ?? 'unknown',
    chunkId: (h.payload['chunk_id'] as number) ?? -1,
    headings: (h.payload['headings'] as string[]) ?? [],
    kind: h.payload['kind'] as string | undefined,
  }));

  if (opts.rerank === false) return candidates.slice(0, topN);

  const reranked = await rerank(query, candidates.map(c => c.text), topN);
  return reranked.map(r => ({ ...candidates[r.index]!, rerankScore: r.score }));
}
```

### 10.3 Verification

```bash
pnpm rag search "What are the components of a RAG pipeline?" --top-k 10 --top-n 3
# Expected: 3 results (sorted by rerankScore) + source path / heading
```

---

## 11. Phase 7: Local LLM Response Generation

### 11.1 Dispatch (`src/llm/index.ts`)

```ts
import { config } from '../config.js';
import { generateOllama, streamOllama } from './ollama.js';
import { generateLlamaCpp, streamLlamaCpp } from './llamacpp.js';
import type { RetrievedDoc } from '../pipeline/retrieve.js';

const SYSTEM_PROMPT = `You are an assistant that answers accurately based on the provided documents.
Strictly follow these rules:
1. Answer based ONLY on the "Reference Information" provided. Do not supplement with imagination.
2. If the answer is not available, explicitly state "The provided information does not contain an answer."
3. List source citations at the end in [1][2] format.
4. Quote numbers and dates exactly as they appear in the original text.`;

export function buildUserMessage(question: string, docs: RetrievedDoc[]): string {
  const ctx = docs.map((d, i) => {
    const path = [d.source, ...d.headings].join(' > ');
    return `[${i + 1}] Source: ${path}\n${d.text}`;
  }).join('\n\n---\n\n');
  return `[Question]\n${question}\n\n[Reference Information]\n${ctx}\n\nPlease answer based on the above information.`;
}

export async function generate(question: string, docs: RetrievedDoc[]): Promise<string> {
  const fn = config.RAG_BACKEND === 'llamacpp' ? generateLlamaCpp : generateOllama;
  return fn(SYSTEM_PROMPT, buildUserMessage(question, docs));
}

export function generateStream(question: string, docs: RetrievedDoc[]): AsyncGenerator<string> {
  const fn = config.RAG_BACKEND === 'llamacpp' ? streamLlamaCpp : streamOllama;
  return fn(SYSTEM_PROMPT, buildUserMessage(question, docs));
}
```

### 11.2 Ollama Path (`src/llm/ollama.ts`)

```ts
import { Ollama } from 'ollama';
import { config } from '../config.js';

const client = new Ollama({ host: config.OLLAMA_HOST });

export async function generateOllama(system: string, user: string): Promise<string> {
  const res = await client.chat({
    model: config.OLLAMA_LLM_MODEL,
    messages: [{ role: 'system', content: system }, { role: 'user', content: user }],
    options: { temperature: 0.1, top_p: 0.9, num_predict: 1024 },
    stream: false,
  });
  return res.message.content;
}

export async function* streamOllama(system: string, user: string): AsyncGenerator<string> {
  const stream = await client.chat({
    model: config.OLLAMA_LLM_MODEL,
    messages: [{ role: 'system', content: system }, { role: 'user', content: user }],
    options: { temperature: 0.1, top_p: 0.9 },
    stream: true,
  });
  for await (const part of stream) {
    if (part.message?.content) yield part.message.content;
  }
}
```

### 11.3 llama.cpp Path (`src/llm/llamacpp.ts`, OpenAI-compatible)

```ts
import { config } from '../config.js';

interface OAResp { choices: Array<{ message?: { content?: string }; delta?: { content?: string } }>; }

export async function generateLlamaCpp(system: string, user: string): Promise<string> {
  const res = await fetch(`${config.LLAMACPP_LLM_URL}/chat/completions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      model: config.LLAMACPP_LLM_MODEL,
      messages: [{ role: 'system', content: system }, { role: 'user', content: user }],
      temperature: 0.1, top_p: 0.9, max_tokens: 1024, stream: false,
    }),
  });
  if (!res.ok) throw new Error(`llamacpp chat failed (${res.status}): ${await res.text()}`);
  const json = await res.json() as OAResp;
  return json.choices[0]?.message?.content ?? '';
}

export async function* streamLlamaCpp(system: string, user: string): AsyncGenerator<string> {
  const res = await fetch(`${config.LLAMACPP_LLM_URL}/chat/completions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      model: config.LLAMACPP_LLM_MODEL,
      messages: [{ role: 'system', content: system }, { role: 'user', content: user }],
      temperature: 0.1, top_p: 0.9, max_tokens: 1024, stream: true,
    }),
  });
  if (!res.body) throw new Error('No stream body');
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    let idx;
    while ((idx = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, idx).trim();
      buf = buf.slice(idx + 1);
      if (!line.startsWith('data:')) continue;
      const payload = line.slice(5).trim();
      if (payload === '[DONE]') return;
      try {
        const j = JSON.parse(payload) as OAResp;
        const delta = j.choices[0]?.delta?.content;
        if (delta) yield delta;
      } catch { /* skip parse errors */ }
    }
  }
}
```

### 11.4 Verification

```bash
pnpm rag search "What are the components of a RAG pipeline?"
# Expected: Japanese response based on ingested documents + sources [1][2]...
```

---

## 12. Phase 8: HTTP API (Hono)

### 12.1 `src/api/server.ts`

```ts
import { serve } from '@hono/node-server';
import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { logger as honoLogger } from 'hono/logger';
import { timeout } from 'hono/timeout';
import { zValidator } from '@hono/zod-validator';
import { streamText } from 'hono/streaming';
import { z } from 'zod';
import { writeFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';

import { config } from '../config.js';
import { logger } from '../logger.js';
import { retrieve } from '../pipeline/retrieve.js';
import { generate, generateStream } from '../llm/index.js';
import { ingestPath, ingestPaths, expandPath } from '../pipeline/ingest.js';
import { ensureCollection, getQdrantClient } from '../search/qdrant.js';

const app = new Hono();
app.use('*', cors());
app.use('*', honoLogger());
app.use('*', timeout(15 * 60 * 1000));

app.get('/health', c => c.json({ status: 'ok' }));

// -- /status: Service health and collection statistics --
app.get('/status', async c => {
  const checks = await Promise.allSettled([
    fetch(`${config.QDRANT_URL}/readyz`).then(r => r.ok),
    fetch(`${config.OLLAMA_HOST}/api/tags`).then(r => r.ok),
    fetch(`${config.DOCLING_URL}/health`).then(r => r.ok),
  ]);
  const client = getQdrantClient();
  const collections = await client.getCollections().then(r => r.collections).catch(() => []);
  return c.json({
    qdrant: checks[0].status === 'fulfilled' && checks[0].value ? 'ok' : 'down',
    ollama: checks[1].status === 'fulfilled' && checks[1].value ? 'ok' : 'down',
    docling: checks[2].status === 'fulfilled' && checks[2].value ? 'ok' : 'down',
    backend: config.RAG_BACKEND,
    collections,
  });
});

// -- /ingest: Ingest path/URL array --
// paths accepts any mix of:
//   - Local file path (PDF / image / SVG / drawio / Markdown / text)
//   - Local directory (recursive enumeration)
//   - URL (http://... / https://...)
//   - .urls file (one URL per line)
const IngestSchema = z.object({
  paths: z.array(z.string().min(1)).min(1),
  collection: z.string().optional(),
});

app.post('/ingest', zValidator('json', IngestSchema), async c => {
  const { paths } = c.req.valid('json');
  const expanded: string[] = [];
  for (const p of paths) expanded.push(...await expandPath(p));
  const stats = await ingestPaths(expanded);
  return c.json({ ...stats, total: expanded.length });
});

// -- /ingest/upload: Multipart file upload --
app.post('/ingest/upload', async c => {
  const body = await c.req.parseBody();
  const file = body['file'];
  if (!(file instanceof File)) return c.json({ error: 'file field required' }, 400);
  await mkdir(join('data', 'upload'), { recursive: true });
  const savedPath = join('data', 'upload', file.name);
  await writeFile(savedPath, Buffer.from(await file.arrayBuffer()));
  const r = await ingestPath(savedPath);
  return c.json({ path: savedPath, ...r });
});

// -- /search: Search + generation --
const SearchSchema = z.object({
  query: z.string().min(1).max(2000),
  top_k: z.number().int().min(1).max(100).optional(),
  top_n: z.number().int().min(1).max(20).optional(),
  rerank: z.boolean().optional(),
  generate: z.boolean().default(true),
});

app.post('/search', zValidator('json', SearchSchema), async c => {
  const { query, top_k, top_n, rerank, generate: gen } = c.req.valid('json');
  const docs = await retrieve(query, { topK: top_k, topN: top_n, rerank });
  const answer = gen ? await generate(query, docs) : null;
  return c.json({ answer, sources: docs });
});

// -- /search/stream: SSE-style stream --
app.post('/search/stream', zValidator('json', SearchSchema), async c => {
  const { query, top_k, top_n, rerank } = c.req.valid('json');
  const docs = await retrieve(query, { topK: top_k, topN: top_n, rerank });
  return streamText(c, async stream => {
    await stream.writeln(JSON.stringify({ type: 'sources', sources: docs }));
    await stream.writeln('---');
    for await (const tok of generateStream(query, docs)) await stream.write(tok);
  });
});

// -- /reindex: Recreate collection --
app.post('/reindex', async c => {
  const client = getQdrantClient();
  await ensureCollection(client, true);
  return c.json({ collection: config.QDRANT_COLLECTION, recreated: true });
});

app.onError((err, c) => {
  logger.error({ err: err.message, stack: err.stack }, 'API error');
  return c.json({ error: err.message }, 500);
});

logger.info({ port: config.RAG_API_PORT, host: config.RAG_API_HOST }, 'Starting Hono server');
serve({ fetch: app.fetch, port: config.RAG_API_PORT, hostname: config.RAG_API_HOST });

export { app };
```

### 12.2 Verification

```bash
# Start server
pnpm serve &
sleep 2

curl -fsS http://127.0.0.1:7777/health
# Expected: {"status":"ok"}

curl -fsS http://127.0.0.1:7777/status | jq
# Expected: { qdrant:"ok", ollama:"ok", docling:"ok", backend:"ollama", collections:[...] }

curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["data/svg"]}'
# Expected: {"ingested":N, "chunks":M, "errors":0, "total":N}

curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"What are the components of a RAG pipeline?","top_k":10,"top_n":3}'
# Expected: { answer: "...", sources: [...] }
```

---

## 13. Phase 9: User CLI (`pnpm rag`)

### 13.1 `src/cli/index.ts`

```ts
import { Command } from 'commander';
import { ingestPath, ingestPaths, expandPath } from '../pipeline/ingest.js';
import { retrieve } from '../pipeline/retrieve.js';
import { generate } from '../llm/index.js';
import { ensureCollection, getQdrantClient } from '../search/qdrant.js';
import { config } from '../config.js';

const program = new Command();
program.name('local-rag').description('Standalone RAG CLI').version('0.1.0');

program
  .command('ingest <target>')
  .description('Ingest file / directory / URL / .urls file (mixed)')
  .action(async (target: string) => {
    const expanded = await expandPath(target);
    if (expanded.length === 1) {
      const r = await ingestPath(expanded[0]!);
      console.log(JSON.stringify({ source: expanded[0], ...r }));
    } else {
      const stats = await ingestPaths(expanded);
      console.log(JSON.stringify({ ...stats, total: expanded.length }, null, 2));
    }
  });

program
  .command('search <query>')
  .description('Search + LLM response')
  .option('-k, --top-k <n>', 'Number of retrieval candidates', '20')
  .option('-n, --top-n <n>', 'Number of results after reranking', '5')
  .option('--no-rerank', 'Disable reranking')
  .option('--no-generate', 'Disable LLM response generation (sources only)')
  .action(async (query: string, opts: { topK: string; topN: string; rerank: boolean; generate: boolean }) => {
    const docs = await retrieve(query, { topK: Number(opts.topK), topN: Number(opts.topN), rerank: opts.rerank });
    if (opts.generate) {
      const answer = await generate(query, docs);
      console.log('=== Answer ===');
      console.log(answer);
    }
    console.log('\n=== Sources ===');
    docs.forEach((d, i) => console.log(`[${i + 1}] ${d.source}${d.headings.length ? ' > ' + d.headings.join(' > ') : ''} (rerank=${d.rerankScore?.toFixed(3) ?? 'n/a'})`));
  });

program
  .command('status')
  .description('Service health and collection statistics')
  .action(async () => {
    const probes = await Promise.allSettled([
      fetch(`${config.QDRANT_URL}/readyz`),
      fetch(`${config.OLLAMA_HOST}/api/tags`),
      fetch(`${config.DOCLING_URL}/health`),
    ]);
    const client = getQdrantClient();
    const collections = await client.getCollections().then(r => r.collections).catch(() => []);
    console.log(JSON.stringify({
      qdrant: probes[0].status === 'fulfilled' && probes[0].value.ok ? 'ok' : 'down',
      ollama: probes[1].status === 'fulfilled' && probes[1].value.ok ? 'ok' : 'down',
      docling: probes[2].status === 'fulfilled' && probes[2].value.ok ? 'ok' : 'down',
      backend: config.RAG_BACKEND,
      collections,
    }, null, 2));
  });

program
  .command('reindex')
  .description('Delete and recreate collection')
  .action(async () => {
    const client = getQdrantClient();
    await ensureCollection(client, true);
    console.log(JSON.stringify({ collection: config.QDRANT_COLLECTION, recreated: true }));
  });

program
  .command('serve')
  .description('Start Hono HTTP API (127.0.0.1:7777)')
  .option('-p, --port <n>', 'Port override')
  .action(async (opts: { port?: string }) => {
    if (opts.port) process.env.RAG_API_PORT = opts.port;
    await import('../api/server.js');
  });

program.parseAsync(process.argv).catch(e => { console.error(e); process.exit(1); });
```

### 13.2 Verification

```bash
pnpm rag --help
# Expected: ingest / search / status / reindex / serve subcommands displayed

pnpm rag status
# Expected: ok/down for each service and collection list

pnpm rag ingest data/svg/hello.svg
pnpm rag ingest data/md/note.md            # Markdown
pnpm rag ingest data/txt/changelog.txt     # Plain text
pnpm rag ingest https://example.com         # Single URL
pnpm rag ingest data/url/refs.urls          # URL list (one URL per line)
pnpm rag ingest ./docs                     # Recursive directory (mixed OK)
pnpm rag search "What are the components of a RAG pipeline?" --top-k 10 --top-n 3
pnpm rag reindex
```

---

## 14. Phase 10: External Application / Client Usage Examples

The RAG subsystem exposes a Hono REST API (`127.0.0.1:7777`), so it can be called from curl / fetch / any HTTP client in any language. This section shows representative client examples.

### 14.1 Shell curl Examples

#### Ingest (`/ingest`)

```bash
# Single file
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./docs/spec.pdf"]}' | jq

# Markdown / text
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./README.md","./CHANGELOG.txt"]}' | jq

# Web URL (single / multiple / mixed)
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["https://qdrant.tech/documentation/","https://docs.docling-project.org/"]}' | jq

# Mixed files + directories + URLs
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./docs","./data/url/refs.urls","https://example.com"]}' | jq

# Multipart upload
curl -fsS -X POST http://127.0.0.1:7777/ingest/upload \
  -F "file=@./design_rag.md" | jq
```

#### Search (`/search`)

```bash
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"<query>","top_k":20,"top_n":5}' | jq

# Sources only (skip LLM generation)
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"<query>","top_n":5,"generate":false}' | jq '.sources'
```

#### Collection Rebuild (`/reindex`)

```bash
curl -fsS -X POST http://127.0.0.1:7777/reindex | jq
# Expected: {"collection":"rag_documents","recreated":true}
```

Re-run `/ingest` after reindexing to repopulate.

#### Status Check (`/status`)

```bash
curl -fsS http://127.0.0.1:7777/status | jq
# Check collections[].points_count etc. for data volume
```

### 14.2 Node.js / TypeScript Client Example

```ts
// Minimal example using fetch from any Node.js project
const RAG = 'http://127.0.0.1:7777';

async function ingest(paths: string[]) {
  const r = await fetch(`${RAG}/ingest`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ paths }),
  });
  return r.json();
}

async function search(query: string, top_k = 20, top_n = 5) {
  const r = await fetch(`${RAG}/search`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ query, top_k, top_n }),
  });
  return r.json() as Promise<{ answer: string; sources: Array<{ source: string; score: number }> }>;
}

console.log(await ingest(['./docs', 'https://example.com']));
console.log(await search('What is the RAG pipeline structure?'));
```

### 14.3 Python Client Example

```python
import requests

RAG = 'http://127.0.0.1:7777'

# Ingest
r = requests.post(f'{RAG}/ingest', json={'paths': ['./docs', 'https://example.com']})
print(r.json())

# Search
r = requests.post(f'{RAG}/search', json={'query': 'What is the RAG pipeline structure?', 'top_k': 20, 'top_n': 5})
data = r.json()
print(data['answer'])
for s in data['sources']:
    print(f"- {s['source']} (score={s.get('rerankScore', s['score']):.3f})")
```

### 14.4 Start / Stop Runbook

```bash
# Start
cd rag-system
docker compose up -d
pnpm serve &       # Start via tsx. Consider daemonizing with pm2 or systemd

# Stop
kill %1            # pnpm serve
docker compose stop

# Status check
curl -fsS http://127.0.0.1:7777/status | jq
```

Example systemd unit file for `pnpm rag serve`:

```ini
[Unit]
Description=Local Standalone RAG API
After=network.target docker.service

[Service]
Type=simple
WorkingDirectory=%h/rag-system
ExecStart=/usr/bin/env pnpm serve
Restart=on-failure

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now local-rag.service
```

### 14.5 Verification

Verify the API is accessible from external clients:

```bash
# 1. Server is running
curl -fsS http://127.0.0.1:7777/health
# Expected: {"status":"ok"}

# 2. Ingest and search round trip
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./README.md"]}' | jq
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"What is the overview of this project?","top_n":3}' | jq
```

> **Network exposure note**: The default `127.0.0.1` binding allows access only from the same host. For LAN/public exposure, follow the security guidelines in Section 16.3 and always use a reverse proxy with authentication.

---

## 15. Phase 11: Operations / Troubleshooting / Evaluation

### 15.1 Logging

| Output | Location |
|--------|-----------|
| Hono | stdout (pino-pretty); with systemd: `journalctl --user -u rag_documents -f` |
| Qdrant | `docker logs rag-qdrant -f` |
| Ollama | `docker logs rag-ollama -f` |
| Docling Serve | `docker logs rag-docling -f` |

### 15.2 Common Issues

| Symptom | Cause | Solution |
|---------|-------|----------|
| `/embeddings` returns 404 | Model not pulled in Ollama | `docker exec rag-ollama ollama pull bge-m3` |
| `Dim mismatch` | Wrong model pulled | Ensure `OLLAMA_EMBED_MODEL` and `EMBED_DIM` match in `.env` |
| Docling Serve OOM | Large multi-page PDF concurrent ingestion | Reduce `p-queue` concurrency to 1 |
| transformers.js is slow | First model download | First run only. Faster after local caching |
| Port 7777 conflict | Another process using the port | `RAG_API_PORT=7780 pnpm serve` |
| External client cannot connect | API not bound / firewall | `RAG_API_HOST=127.0.0.1` assumes same host. For other hosts, use `0.0.0.0` + reverse proxy |
| drawio `pako.inflateRaw` failure | Uncompressed plain-text mxGraph | Guard check in `decompressDiagram` handles this; usually not an issue |
| Japanese chunks are too small | `chunkSize` is too small | Increase `CHUNK_SIZE` in `.env` to 1024 etc. |

### 15.3 Evaluation

Create a simple evaluation set (question + expected source) in `rag/eval/qa.jsonl` and measure hit rate with `pnpm rag search`:

```jsonl
{"q":"What is the overview of this project?","expect_source":"README.md"}
{"q":"Where is the configuration file?","expect_source":".env.example"}
```

```bash
# Evaluator can be implemented separately (jq + while read is sufficient). See appendix for details.
```

### 15.4 Backup

```bash
# Qdrant snapshot
curl -fsS -X POST http://127.0.0.1:6333/collections/rag_documents/snapshots
# -> Tar is generated in /qdrant/storage/snapshots

# Docker volume backup
docker run --rm -v rag-qdrant-data:/data -v $(pwd):/backup busybox \
  tar cvf /backup/qdrant-$(date +%F).tar /data
```

---

## 16. Appendix

### 16.1 Alternative Stack

| Layer | Primary | Alternative |
|-------|---------|-------------|
| Vector DB | Qdrant | Milvus / Weaviate (not supported in this guide, configuration examples only) |
| Embedding | Ollama bge-m3 | llama.cpp bge-m3 / `multilingual-e5-large` |
| LLM | Ollama qwen2.5 | llama.cpp qwen2.5 / `llama-3.1-8b-instruct` / `gemma-2-9b-it` |
| Reranker | bge-reranker-v2-m3 (ONNX) | `cross-encoder/ms-marco-MiniLM-L-12-v2` |
| Document conversion | Docling Serve | `unstructured.io` API (Python-based, not supported in this guide) |
| API | Hono | Fastify / Express |

### 16.2 Sparse / Hybrid Search

See the reference guide `pdf_image_rag_guide_nodejs.md` Phase 9 (BM25 hybrid). Use `wink-bm25-text-search` + `kuromoji.js` to build sparse vectors and combine with Dense via Qdrant's Multi-Vector. Out of scope for this guide (extend as needed).

### 16.3 Security Considerations

- The API defaults to **`127.0.0.1` binding**. For LAN/public exposure, use a reverse proxy (nginx / caddy) + authentication.
- Docker Compose ports also use the `127.0.0.1:` prefix to restrict to loopback.
- Documents under `data/` are treated as confidential. Added to `.gitignore`; Qdrant snapshots should receive equivalent confidentiality treatment.
- LLM prompts do not embed credentials (only source paths, designed so secrets are not extracted).
- To block HuggingFace model downloads in restricted environments, pre-sync `~/.cache/huggingface/hub`.

### 16.4 Performance Tuning

| Parameter | Default | Increase Effect | Decrease Effect |
|-----------|---------|----------------|-----------------|
| `CHUNK_SIZE` | 512 | Wider context, higher recall | Higher precision, better search accuracy |
| `CHUNK_OVERLAP` | 64 | Less boundary loss | Smaller DB size |
| `TOP_K_RETRIEVE` | 20 | Higher recall (more reranker load) | Lower latency |
| `TOP_K_RERANK` | 5 | More context | Less LLM input |
| `hnsw.m` | 16 | Higher recall | Less memory |
| `ef_construct` | 128 | Better build accuracy | Faster build |
| Qdrant `on_disk` | false | Less RAM | Higher latency |

### 16.5 Summary of Inherited Sections from Reference Guide

| This guide | Reference guide section |
|-----------|------------------------|
| Section 2 Requirements | Reference guide Section 3 |
| Section 4 Phase 0 | Reference guide Phase 0 |
| Section 5 Phase 1 | Reference guide Phase 1 |
| Section 6.2 PDF conversion | Reference guide Phase 2 (Section 2.2) |
| Section 7 Chunking | Reference guide Phase 3, Japanese-enhanced |
| Section 8 Embedding | Reference guide Phase 4 |
| Section 9 Qdrant | Reference guide Phase 5 |
| Section 10 Search + Reranking | Reference guide Phase 6 |
| Section 11 LLM Generation | Reference guide Phase 7 |
| Section 12 Hono API | Reference guide Phase 8, adapted for external client integration |
| Section 16.2 Sparse / Hybrid | Reference guide Phase 9 |
| Section 15 Operations | Reference guide Phase 10 |

### 16.6 Completion Checklist

- [ ] Created project under `rag-system/`
- [ ] `docker compose up -d` starts Qdrant / Ollama / Docling Serve
- [ ] Pulled `bge-m3` / `qwen2.5:7b-instruct` into Ollama
- [ ] `pnpm rag ingest data/svg/<sample>.svg` completes successfully
- [ ] `pnpm rag ingest data/drawio/<sample>.drawio` completes successfully
- [ ] `pnpm rag ingest data/pdf/<sample>.pdf` completes successfully
- [ ] `pnpm rag ingest data/md/<sample>.md` completes successfully
- [ ] `pnpm rag ingest data/txt/<sample>.txt` completes successfully
- [ ] `pnpm rag ingest https://example.com` completes successfully
- [ ] `pnpm rag ingest data/url/refs.urls` bulk URL ingestion completes successfully
- [ ] `pnpm rag search "<query>"` returns answer + sources
- [ ] `curl http://127.0.0.1:7777/health` returns `{"status":"ok"}`
- [ ] External clients (curl / fetch / Python etc.) can call `/ingest` and `/search`
- [ ] `RAG_BACKEND=llamacpp` switches to the llama.cpp path (optional)

---

**End of guide** -- For build questions or additional requirements, carry them to the next session via `.aiprj/instructions.md`.