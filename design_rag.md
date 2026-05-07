# スタンドアロン RAG 環境 構築ガイド

> **対象読者**: ローカル環境で完結するスタンドアロン RAG サブシステムを構築したいエンジニア
> **最終更新**: 2026-05-06
> **配置**: 任意（`./design_rag.md` として参照）
> **参照解**: `./pdf_image_rag_guide_nodejs.md`（1640 行、本ガイドの土台）

---

## TL;DR（30 行未満で要点）

- 単一 Markdown ガイドで **ローカル環境に閉じたスタンドアロン RAG サブシステム**（プロジェクトディレクトリ: `rag-system/`）を構築する。
- スタックは **Node.js 20+ / pnpm 9+ / Docker Compose v2 / Qdrant / Ollama（または llama.cpp）/ Docling Serve / Hono / transformers.js**。外部 SaaS への依存はゼロ。
- 取込は **PDF / 画像 / SVG / drawio / Markdown / プレーンテキスト / Web (URL)** に対応。SVG・drawio は本ガイドで新規追加した XML パーサ + ラベル抽出経路で、Web は Docling Serve `/v1alpha/convert/source` の URL 入力経路で扱う。
- 外部アプリ / CLI / シェルスクリプト等の任意のクライアントから、`127.0.0.1:7777` の Hono REST API を叩いて利用する。
- ユーザは shell から `pnpm rag {ingest|search|status|reindex|serve} ...` で全機能にアクセスできる。
- 日本語対応: 埋め込みは多言語 `bge-m3`（1024 dim）、LLM は `qwen2.5:7b-instruct`、チャンキングは `。、！？` を優先 separator に追加した日本語強化版。
- 本ガイドは参照ガイド `pdf_image_rag_guide_nodejs.md` を **継承 + 上書き + 追加** で構成しており、各章末に検証コマンドと期待結果を併記する。

---

## 0. 参照ガイドとの関係

### 0.1 継承 / 上書き / 追加 対比表

| 章 | 内容 | 参照ガイドからの関係 |
|----|------|-------------------|
| 1 | アーキテクチャ概観 | 一部上書き |
| 2 | 動作要件 | 継承 + llama.cpp 経路を追加 |
| 3 | ディレクトリ配置（`rag-system/` 配下に閉じる） | **新規** |
| 4 | Phase 0: プロジェクト初期化 | 継承（pnpm / TypeScript / .gitignore） |
| 5 | Phase 1: Docker Compose | 継承 |
| 6 | Phase 2: ドキュメント変換（**SVG / drawio / Markdown / テキスト / Web 追加**） | 一部新規 |
| 7 | Phase 3: チャンキング（日本語強化） | 一部上書き |
| 8 | Phase 4: 埋め込み生成（**llama.cpp 経路追加**） | 一部新規 |
| 9 | Phase 5: Qdrant 投入 | 継承 |
| 10 | Phase 6: 検索 + リランキング | 継承 |
| 11 | Phase 7: ローカル LLM 応答（**llama.cpp 経路追加**） | 一部新規 |
| 12 | Phase 8: HTTP API（Hono） | 一部上書き |
| 13 | Phase 9: ユーザ CLI（`pnpm rag` サブコマンド） | **新規** |
| 14 | Phase 10: 外部アプリ / クライアントからの利用例 | **新規** |
| 15 | Phase 11: 運用・トラブルシューティング | 継承 |
| 16 | 付録（代替スタック / 性能 / セキュリティ） | 一部上書き |

### 0.2 本ガイドの位置付け

参照ガイド `pdf_image_rag_guide_nodejs.md` は **汎用** な PDF/画像 RAG 構築手順である。本ガイドはそれを単独運用向けに整理し、以下を必ず付加する:

1. 取込フォーマットに **SVG / drawio / Markdown / プレーンテキスト / Web (URL)** を追加
2. 推論バックエンドに **llama.cpp** を Ollama と並走可能にする（Ollama が動かない環境向け）
3. **HTTP REST API**（Hono）で外部アプリ / CLI / 任意クライアントから利用可能にする
4. **ユーザ CLI**（`pnpm rag`）で shell から直接操作
5. **日本語強化**（句読点 `。、！？` を優先 separator に追加、qwen2.5 を既定）

参照ガイドの章 / コードを引用するときは **章番号で参照** し、本ガイド単独で構築完了できる粒度に保つ。

---

## 1. アーキテクチャ概観

### 1.1 全体図

```
┌──────────────────────────────────────────────────────────────────────┐
│  外部クライアント（任意）                                                │
│   ─ ユーザ shell / CLI / Web フロント / 他アプリ / シェルスクリプト等       │
│   ─ curl / fetch / SDK で HTTP を発行                                  │
└──────────────────────┬───────────────────────────────────────────────┘
                       │ HTTP (127.0.0.1:7777)
                       ▼
┌──────────────────────────────────────────────────────────────────────┐
│         Hono API (Node.js / pnpm — rag-system/)                       │
│   POST /ingest    POST /search    GET /status    POST /reindex        │
│                                                                       │
│   pnpm rag {ingest|search|status|reindex|serve}                       │
└──────┬──────────────┬───────────────┬───────────────┬─────────────────┘
       │              │               │               │
       ▼              ▼               ▼               ▼
  Docling Serve   Ollama          Qdrant         transformers.js
  (PDF→MD)        (bge-m3 emb     (vector DB)    (bge-reranker-v2-m3)
                   + qwen2.5
                   LLM)            collection:
                                   rag_documents
       │
       │ ← llama.cpp (llama-server) を Ollama の代替として併走可
       ▼
   /v1/chat/completions / /v1/embeddings (OpenAI 互換)
```

### 1.2 データフロー

**取込（オフライン）**:
```
  PDF / 画像 / SVG / drawio / Markdown / テキスト / Web URL
     │
     ▼
  形式判定 (extension / mime / URL スキーム)
     │
     ├─ PDF / 画像          → Docling Serve (REST /convert/file) → Markdown
     ├─ SVG / .drawio.svg   → fast-xml-parser → text/title/desc 抽出 → Markdown
     ├─ drawio (.drawio)    → pako.inflateRaw → mxGraph XML → mxCell.value 抽出 → Markdown
     ├─ Markdown / テキスト  → 直接読込み（必要に応じ frontmatter 除去）
     └─ Web URL (http(s)://)→ Docling Serve (REST /convert/source URL) → Markdown
     │
     ▼
  日本語強化チャンキング（句読点 separator）
     │
     ▼
  Ollama bge-m3 (or llama.cpp embeddings) → 1024 dim Dense ベクトル
     │
     ▼
  Qdrant upsert (collection: rag_documents)
```

**検索（オンライン）**:
```
  ユーザ質問 (CLI / Hono /search / 任意クライアント)
     │
     ▼
  bge-m3 でクエリ埋め込み
     │
     ▼
  Qdrant Dense 検索 (top_k=20)
     │
     ▼
  transformers.js bge-reranker-v2-m3 で rerank (top_n=5)
     │
     ▼
  qwen2.5 (Ollama / llama.cpp) で回答生成
     │
     ▼
  応答 + 出典（path / page / heading path / score）
```

### 1.3 コンポーネント責務

| レイヤー | 技術 | 配置 |
|--------|------|------|
| ドキュメント変換（PDF / 画像 / Web URL） | **Docling Serve** | Docker、Node から HTTP（`/convert/file` または `/convert/source`） |
| ドキュメント変換（SVG / drawio） | `fast-xml-parser` + `pako` | Node プロセス内 |
| ドキュメント変換（Markdown / テキスト） | `node:fs` + `gray-matter` | Node プロセス内（変換不要、frontmatter 分離のみ） |
| チャンキング | `@langchain/textsplitters` + 日本語句読点拡張 | Node プロセス内 |
| 埋め込み | `bge-m3` on **Ollama** または **llama.cpp llama-server** | Docker / バイナリ、Node から HTTP |
| ベクトル DB | **Qdrant** | Docker、Node から HTTP |
| リランカ | `@huggingface/transformers` (ONNX) | Node プロセス内 |
| LLM | `qwen2.5` on **Ollama** または **llama.cpp** | 同上 |
| API | **Hono** | Node プロセス、`127.0.0.1:7777` |
| CLI | `commander` | Node プロセス、`pnpm rag` |
| 外部連携 | curl / fetch / 任意 HTTP クライアント | 既存資産への影響なし |

---

## 2. 動作要件

### 2.1 ハードウェア / ソフトウェア

| 項目 | 要件 |
|-----|------|
| OS | Linux (Ubuntu 22.04+ 推奨) / macOS (Ollama 経路) |
| CPU | 8 コア以上 |
| RAM | 32 GB（Ollama LLM 推論時は 16 GB 以上推奨） |
| GPU | 任意（NVIDIA + CUDA 12 系で Ollama / llama.cpp 加速） |
| ストレージ | SSD 100 GB 以上（モデル + Qdrant データ + Docling キャッシュ） |
| Node.js | **20.x 以上**（LTS 推奨） |
| pnpm | **9.x 以上**（`npm install -g pnpm`） |
| Docker | 24.0+, Docker Compose v2.20+ |
| Ollama | 0.5.x 以上（Docker 経由 or ホスト直） |
| llama.cpp | `llama-server` バイナリ（任意。副系統） |

### 2.2 ソフトウェア前提確認

```bash
node --version            # v20.0.0 以上
pnpm --version            # 9.0 以上
docker --version
docker compose version
nvidia-smi                # GPU 利用時
```

### 2.3 ポート使用一覧

| サービス | ポート | 設定変数 |
|---------|--------|--------|
| Qdrant REST | 6333 | `QDRANT_URL` |
| Qdrant gRPC | 6334 | - |
| Ollama | 11434 | `OLLAMA_HOST` |
| Docling Serve | 5001 | `DOCLING_URL` |
| llama.cpp llama-server (副) | 8080（embeddings） / 8081（LLM） | `LLAMACPP_EMBED_URL` / `LLAMACPP_LLM_URL` |
| **Hono API**（外部クライアント連携用） | **7777** | `RAG_API_PORT` |

ポート衝突時は環境変数で上書きする。確認:
```bash
ss -ltn '( sport = :7777 or sport = :6333 or sport = :11434 or sport = :5001 )'
```

---

## 3. ディレクトリ配置

RAG サブシステムは **単一プロジェクトディレクトリ** `rag-system/` 配下に閉じる。設置先は任意で、ユーザの `$HOME/projects/` でも、既存リポジトリの 1 サブディレクトリでもよい。

```
rag-system/                            ← RAG サブシステム本体（任意の親 dir に設置）
├── design_rag.md                      ← 本ガイド（任意配置）
├── pdf_image_rag_guide_nodejs.md      ← 参照ガイド（任意配置、不変）
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── docker-compose.yml
├── .env.example
├── .gitignore
├── Makefile
├── src/
│   ├── config.ts                      ← 環境変数ロード
│   ├── logger.ts
│   ├── api/
│   │   └── server.ts                  ← Hono サーバ (port 7777)
│   ├── cli/
│   │   └── index.ts                   ← `pnpm rag` エントリ
│   ├── ingest/
│   │   ├── index.ts                   ← 取込ディスパッチ（拡張子 / URL 判定）
│   │   ├── pdf.ts                     ← Docling Serve `/convert/file`
│   │   ├── svg.ts                     ← SVG XML パース（新規）
│   │   ├── drawio.ts                  ← drawio mxGraph パース（新規）
│   │   ├── markdown.ts                ← Markdown / テキスト直接読込み（新規）
│   │   └── web.ts                     ← Web URL → Docling Serve `/convert/source`（新規）
│   ├── chunk/
│   │   └── japanese.ts                ← 日本語句読点考慮
│   ├── embed/
│   │   ├── index.ts                   ← バックエンド切替
│   │   ├── ollama.ts
│   │   └── llamacpp.ts                ← OpenAI 互換 /v1/embeddings
│   ├── search/
│   │   ├── qdrant.ts
│   │   └── rerank.ts
│   ├── llm/
│   │   ├── index.ts
│   │   ├── ollama.ts
│   │   └── llamacpp.ts                ← OpenAI 互換 /v1/chat/completions
│   └── pipeline/
│       ├── ingest.ts                  ← 取込パイプライン
│       └── retrieve.ts                ← 検索パイプライン
└── data/                              ← .gitignore（バイナリ・モデル除く）
    ├── pdf/
    ├── svg/
    ├── drawio/
    ├── md/                            ← 直接取込み Markdown
    ├── txt/                           ← プレーンテキスト
    ├── url/                           ← URL 一覧（`*.urls` 形式テキスト、各行 1 URL）
    └── markdown/                      ← 変換結果（中間生成物）
```

### 3.1 .gitignore（`rag/.gitignore`）

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

## 4. Phase 0: プロジェクト初期化

### 4.1 ディレクトリ作成

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

### 4.3 依存パッケージ

**ランタイム**:

```bash
# HTTP / API
pnpm add hono @hono/node-server @hono/zod-validator

# 推論クライアント
pnpm add ollama
# llama.cpp 経路は標準 fetch + OpenAI 互換のため追加依存なし

# ベクトル DB
pnpm add @qdrant/js-client-rest

# リランカ
pnpm add @huggingface/transformers

# テキスト処理
pnpm add @langchain/textsplitters gray-matter

# XML / 圧縮（SVG / drawio 用）
pnpm add fast-xml-parser pako

# 画像（SVG OCR フォールバック、任意）
pnpm add sharp

# ユーティリティ
pnpm add zod dotenv pino pino-pretty commander p-queue undici mime-types
```

**開発依存**:

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

### 4.5 環境変数（`.env.example`）

```ini
# === Qdrant ===
QDRANT_URL=http://127.0.0.1:6333
QDRANT_API_KEY=
QDRANT_COLLECTION=rag_documents

# === 推論バックエンド切替 ===
RAG_BACKEND=ollama        # ollama | llamacpp

# === Ollama ===
OLLAMA_HOST=http://127.0.0.1:11434
OLLAMA_LLM_MODEL=qwen2.5:7b-instruct
OLLAMA_EMBED_MODEL=bge-m3

# === llama.cpp (副系統、OpenAI 互換) ===
LLAMACPP_EMBED_URL=http://127.0.0.1:8080/v1
LLAMACPP_LLM_URL=http://127.0.0.1:8081/v1
LLAMACPP_EMBED_MODEL=bge-m3
LLAMACPP_LLM_MODEL=qwen2.5-7b-instruct

# === Docling Serve ===
DOCLING_URL=http://127.0.0.1:5001

# === リランカー ===
RERANKER_MODEL=onnx-community/bge-reranker-v2-m3-ONNX

# === Hono API ===
RAG_API_HOST=127.0.0.1
RAG_API_PORT=7777

# === ハイパーパラメータ ===
CHUNK_SIZE=512
CHUNK_OVERLAP=64
TOP_K_RETRIEVE=20
TOP_K_RERANK=5
EMBED_DIM=1024
LOG_LEVEL=info
```

### 4.6 設定ローダー（`src/config.ts`）

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

### 4.7 ロガー（`src/logger.ts`）

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

### 4.8 検証

```bash
cd rag-system
pnpm typecheck
# 期待: エラーなし（src/ がまだ空でも tsconfig が解釈できれば OK）
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
    # GPU が無ければ deploy セクションを削除
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]

  docling:
    image: quay.io/docling-project/docling-serve-cpu:latest
    # GPU 版: quay.io/docling-project/docling-serve-cu128:latest
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

### 5.2 起動とモデル投入

```bash
cd rag-system
docker compose up -d
docker compose ps

# 稼働確認
curl -fsS http://127.0.0.1:6333/readyz       # Qdrant
curl -fsS http://127.0.0.1:11434/api/tags    # Ollama
curl -fsS http://127.0.0.1:5001/health       # Docling Serve

# Ollama にモデル投入
docker exec rag-ollama ollama pull qwen2.5:7b-instruct
docker exec rag-ollama ollama pull bge-m3
docker exec rag-ollama ollama list
```

### 5.3 検証

```bash
curl -fsS http://127.0.0.1:6333/readyz
# 期待: 200 OK + ステータス JSON

curl -fsS -X POST http://127.0.0.1:11434/api/embed \
  -d '{"model":"bge-m3","input":"日本語テスト"}' | jq '.embeddings[0] | length'
# 期待: 1024
```

### 5.4 llama.cpp 副系統（任意）

llama.cpp の `llama-server` を 2 ポートで起動（GGUF モデルを別途取得済の前提）:

```bash
# 埋め込み (port 8080)
llama-server -m models/bge-m3-Q5_K_M.gguf --port 8080 --embeddings &

# LLM (port 8081)
llama-server -m models/qwen2.5-7b-instruct-Q5_K_M.gguf --port 8081 -c 8192 &
```

両者とも OpenAI 互換 (`/v1/embeddings`, `/v1/chat/completions`) を提供するため、`.env` の `RAG_BACKEND=llamacpp` で切り替えるだけで本ガイドのコードがそのまま動く。

---

## 6. Phase 2: ドキュメント変換（PDF / 画像 / SVG / drawio / Markdown / テキスト / Web）

### 6.1 変換ディスパッチ（`src/ingest/index.ts`）

入力は **ローカルファイルパス** または **URL（`http://` / `https://`）** を許容する。`.urls` 拡張子のテキストファイルを「URL リスト」として扱い、行ごとに展開する経路も用意する。

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

### 6.2 PDF / 画像（Docling Serve、参照ガイド継承）

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

### 6.3 SVG 取込（**新規**）

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

  // .drawio.svg は <svg content="..."> に mxGraph XML が埋め込まれている場合あり
  // → drawio.ts と共用するためここでも展開を試みる
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

### 6.4 drawio 取込（**新規**）

`src/ingest/drawio.ts`:

```ts
import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { XMLParser } from 'fast-xml-parser';
import pako from 'pako';
import type { ConvertedDoc } from './index.js';

/**
 * drawio の <diagram> 要素の中身は次のいずれか:
 *   1) 平文 mxGraph XML (<mxGraphModel>...</mxGraphModel>)
 *   2) deflate + base64 でエンコードされた URL-encoded mxGraph XML
 */
export function decompressDiagram(content: string): string {
  const trimmed = content.trim();
  if (trimmed.startsWith('<mxGraphModel') || trimmed.startsWith('<?xml')) {
    return trimmed;
  }
  // base64 → bytes → inflateRaw → URL decode
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

  // 多段ネスト: mxfile > diagram[*] > (compressed | mxGraphModel)
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

### 6.5 Markdown / プレーンテキスト取込（**新規**）

`src/ingest/markdown.ts`:

```ts
import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import matter from 'gray-matter';
import type { ConvertedDoc } from './index.js';

/**
 * Markdown はそのまま投入する。frontmatter は metadata に分離。
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
 * プレーンテキスト / .log / .rst は最低限の整形だけ行い Markdown 同等に扱う。
 * - BOM 除去
 * - CRLF → LF
 * - 連続空行を 1 行に圧縮（過剰チャンク化防止）
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

### 6.6 Web (URL) 取込（**新規**）

Docling Serve は **URL を直接受け取る** `/v1alpha/convert/source` エンドポイントを備えており、HTML / PDF / 画像のいずれの URL もサーバ側で取得 + 解析 + Markdown 化する。Node 側で fetch + cheerio + html-to-md を組む必要は無い。

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
 * URL を Docling Serve に渡し Markdown を取得する。
 * - HTTP / HTTPS のみ許可
 * - HTML / PDF / 画像のいずれでも 1 経路で扱える
 * - OCR は HTML では発火しないが、PDF / 画像 URL では自動で発火
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
 * `.urls` 拡張子のテキストファイル（行ごと 1 URL、`#` で始まる行はコメント）を読み URL 配列を返す。
 * `expandPath` で展開され、各 URL は `convertWeb` で個別取込される。
 */
export async function readUrlList(filePath: string): Promise<string[]> {
  const { readFile } = await import('node:fs/promises');
  const raw = await readFile(filePath, 'utf-8');
  return raw.split(/\r?\n/).map(l => l.trim()).filter(l => l && !l.startsWith('#'));
}
```

#### Web 取込の留意点

- **対象範囲**: 単一 URL を 1 ドキュメントとして取込む。サイト全体のクロール（深さ N の再帰巡回）はスコープ外。クロールが必要なら `wget --mirror` 等で事前にローカルへ落としてからファイル取込する運用を推奨。
- **認証**: 認証付き URL は Docling Serve から直接アクセス不可。Cookie ヘッダ送信が必要なら `wget` などで先にダウンロード → ファイル取込する。
- **大量 URL**: `.urls` ファイル（行ごと 1 URL）を `pnpm rag ingest path/to/list.urls` で一括投入できる（後述 §13）。
- **更新**: 同 URL を再取込すると新しいチャンクが追加される（重複）。再取込時は `pnpm rag reindex` で collection をクリアしてから再投入するか、`payload.url` をキーに古い point を削除する運用を推奨。

### 6.7 検証

```bash
# SVG: <text>Hello</text> を含むサンプル
cat > rag-system/data/svg/hello.svg <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg"><text x="10" y="20">こんにちは世界</text></svg>
EOF
pnpm tsx -e "import('./src/ingest/svg.js').then(m => m.convertSvg('data/svg/hello.svg').then(r => console.log(r.markdown)))"
# 期待: "- [text] (x=10, y=20) こんにちは世界" を含む Markdown

# drawio: 圧縮ありのファイルを drawio Web で出力したものを配置して動作確認
ls rag-system/data/drawio/*.drawio
pnpm tsx -e "import('./src/ingest/drawio.js').then(m => m.convertDrawio('data/drawio/sample.drawio').then(r => console.log(r.markdown.slice(0, 500))))"
# 期待: "# drawio: sample.drawio" + "## Diagram 1" + ラベル一覧

# Markdown: 直接読込
cat > rag-system/data/md/note.md <<'EOF'
---
title: メモ
---
# 概要

本書はサンプルメモ。
EOF
pnpm tsx -e "import('./src/ingest/markdown.js').then(m => m.convertMarkdown('data/md/note.md').then(r => console.log(JSON.stringify(r, null, 2))))"
# 期待: metadata.frontmatter.title === 'メモ'、markdown 本文に "# 概要" を含む

# プレーンテキスト
cat > rag-system/data/txt/changelog.txt <<'EOF'
2026-05-06: 取込パイプラインを追加。
2026-05-07: web 取込対応。
EOF
pnpm tsx -e "import('./src/ingest/markdown.js').then(m => m.convertText('data/txt/changelog.txt').then(r => console.log(r.markdown)))"
# 期待: "# changelog.txt" + 本文 2 行

# Web URL（Docling Serve 起動済が前提）
pnpm tsx -e "import('./src/ingest/web.js').then(m => m.convertWeb('https://example.com').then(r => console.log(r.markdown.slice(0, 200))))"
# 期待: "Example Domain" など example.com の本文を Markdown として取得

# URL リスト
cat > rag-system/data/url/refs.urls <<'EOF'
# 参考リンク集（コメント行は無視される）
https://example.com
https://qdrant.tech/documentation/
EOF
pnpm tsx -e "import('./src/ingest/web.js').then(async m => console.log(await m.readUrlList('data/url/refs.urls')))"
# 期待: ['https://example.com', 'https://qdrant.tech/documentation/']
```

---

## 7. Phase 3: チャンキング（日本語強化）

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
  '\n\n',  // パラグラフ
  '\n',    // 行
  '。', '！', '？',  // 日本語句点
  '. ', '! ', '? ', // 英文末
  '、',                // 日本語読点（最終手段）
  ' ',                 // 半角空白
  '',                  // 文字単位
];

export async function chunkJapanese(
  source: string,
  markdown: string,
): Promise<Chunk[]> {
  const { content, data: frontmatter } = matter(markdown);

  // 見出し構造で粗く分割
  const mdSplitter = new MarkdownTextSplitter({
    chunkSize: config.CHUNK_SIZE * 4,
    chunkOverlap: 0,
  });
  const sections = await mdSplitter.splitText(content);

  // 日本語句読点優先で細粒度分割
  const refine = new RecursiveCharacterTextSplitter({
    chunkSize: config.CHUNK_SIZE * 3,        // 1 トークン ≒ 2-3 文字（bge-m3 日本語）
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
      if (text.trim().length < 8) continue; // 過小チャンク除外
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

### 7.2 検証

```bash
pnpm tsx -e "
import('./src/chunk/japanese.js').then(async m => {
  const chunks = await m.chunkJapanese('test.md', '# 概要\n\n本システムは RAG パイプラインです。日本語ドキュメントを取り込みます。検索とリランクで上位を返します。');
  console.log('chunks:', chunks.length);
  for (const c of chunks) console.log('-', c.text.slice(0, 80));
})"
# 期待: 1〜3 chunks、本文先頭に "# 概要" が付与
```

---

## 8. Phase 4: 埋め込み生成（Ollama 主 / llama.cpp 副）

### 8.1 ディスパッチ（`src/embed/index.ts`）

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

### 8.2 Ollama 経路（`src/embed/ollama.ts`）

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

### 8.3 llama.cpp 経路（`src/embed/llamacpp.ts`、OpenAI 互換）

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

### 8.4 検証

```bash
RAG_BACKEND=ollama pnpm tsx -e "
import('./src/embed/index.js').then(async m => {
  const v = await m.embedOne('RAG パイプラインの構成要素は何ですか？');
  console.log('dim:', v.length);
})"
# 期待: dim: 1024

# llama.cpp 経路（llama-server 起動時のみ）
RAG_BACKEND=llamacpp pnpm tsx -e "
import('./src/embed/index.js').then(async m => {
  const v = await m.embedOne('テスト');
  console.log('dim:', v.length);
})"
```

---

## 9. Phase 5: Qdrant 投入

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

### 9.2 取込パイプライン（`src/pipeline/ingest.ts`）

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
 * 入力 1 件を ingest 対象に展開する:
 *   - URL (`http(s)://...`)         → そのまま 1 件
 *   - `.urls` ファイル              → 行ごと 1 URL に展開
 *   - 単一ファイル                  → そのまま 1 件
 *   - ディレクトリ                  → 配下の対応拡張子を再帰列挙
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

### 9.3 検証

```bash
curl -fsS http://127.0.0.1:6333/collections | jq
# 期待: 取込前は空、取込後は { "collections": [{"name":"rag_documents"}] }

# 簡単な PDF / SVG を data/ に置いて
pnpm rag ingest data/svg/hello.svg
curl -fsS http://127.0.0.1:6333/collections/rag_documents | jq '.result.points_count'
# 期待: 1 以上
```

---

## 10. Phase 6: 検索 + リランキング

### 10.1 リランカ（`src/search/rerank.ts`）

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

### 10.2 検索パイプライン（`src/pipeline/retrieve.ts`）

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

### 10.3 検証

```bash
pnpm rag search "RAG パイプラインの構成要素は？" --top-k 10 --top-n 3
# 期待: 3 件の結果（rerankScore 降順）+ 出典 path / heading
```

---

## 11. Phase 7: ローカル LLM 応答生成

### 11.1 ディスパッチ（`src/llm/index.ts`）

```ts
import { config } from '../config.js';
import { generateOllama, streamOllama } from './ollama.js';
import { generateLlamaCpp, streamLlamaCpp } from './llamacpp.js';
import type { RetrievedDoc } from '../pipeline/retrieve.js';

const SYSTEM_PROMPT = `あなたは提供されたドキュメントに基づいて正確に回答するアシスタントです。
以下を厳守:
1. 「参考情報」のみに基づいて回答する。想像で補わない。
2. 答えがない場合は「提供された情報では回答できません」と明言する。
3. 末尾に [1][2] 形式で出典番号を列挙する。
4. 数値・日付は原文どおりに引用する。`;

export function buildUserMessage(question: string, docs: RetrievedDoc[]): string {
  const ctx = docs.map((d, i) => {
    const path = [d.source, ...d.headings].join(' > ');
    return `[${i + 1}] 出典: ${path}\n${d.text}`;
  }).join('\n\n---\n\n');
  return `【質問】\n${question}\n\n【参考情報】\n${ctx}\n\n上記に基づいて回答してください。`;
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

### 11.2 Ollama 経路（`src/llm/ollama.ts`）

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

### 11.3 llama.cpp 経路（`src/llm/llamacpp.ts`、OpenAI 互換）

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

### 11.4 検証

```bash
pnpm rag search "RAG パイプラインの構成要素は？"
# 期待: 取込済ドキュメントに基づいた日本語応答 + 出典 [1][2]...
```

---

## 12. Phase 8: HTTP API（Hono）

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

// ── /status: 各サービスのヘルスと collection 統計 ──
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

// ── /ingest: パス / URL 配列を取込 ──
// paths は以下のいずれも混在可:
//   - ローカルファイルパス (PDF / 画像 / SVG / drawio / Markdown / テキスト)
//   - ローカルディレクトリ（再帰列挙）
//   - URL (http://... / https://...)
//   - .urls ファイル（行ごとに URL）
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

// ── /ingest/upload: multipart ファイルアップロード ──
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

// ── /search: 検索 + 生成 ──
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

// ── /search/stream: SSE 風ストリーム ──
app.post('/search/stream', zValidator('json', SearchSchema), async c => {
  const { query, top_k, top_n, rerank } = c.req.valid('json');
  const docs = await retrieve(query, { topK: top_k, topN: top_n, rerank });
  return streamText(c, async stream => {
    await stream.writeln(JSON.stringify({ type: 'sources', sources: docs }));
    await stream.writeln('---');
    for await (const tok of generateStream(query, docs)) await stream.write(tok);
  });
});

// ── /reindex: collection 再作成 ──
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

### 12.2 検証

```bash
# サーバ起動
pnpm serve &
sleep 2

curl -fsS http://127.0.0.1:7777/health
# 期待: {"status":"ok"}

curl -fsS http://127.0.0.1:7777/status | jq
# 期待: { qdrant:"ok", ollama:"ok", docling:"ok", backend:"ollama", collections:[...] }

curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["data/svg"]}'
# 期待: {"ingested":N, "chunks":M, "errors":0, "total":N}

curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"RAG パイプラインの構成要素は？","top_k":10,"top_n":3}'
# 期待: { answer: "...", sources: [...] }
```

---

## 13. Phase 9: ユーザ CLI（`pnpm rag`）

### 13.1 `src/cli/index.ts`

```ts
import { Command } from 'commander';
import { ingestPath, ingestPaths, expandPath } from '../pipeline/ingest.js';
import { retrieve } from '../pipeline/retrieve.js';
import { generate } from '../llm/index.js';
import { ensureCollection, getQdrantClient } from '../search/qdrant.js';
import { config } from '../config.js';

const program = new Command();
program.name('local-rag').description('スタンドアロン RAG CLI').version('0.1.0');

program
  .command('ingest <target>')
  .description('ファイル / ディレクトリ / URL / .urls ファイル を取込（混在可）')
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
  .description('検索 + LLM 応答')
  .option('-k, --top-k <n>', 'retrieve 候補数', '20')
  .option('-n, --top-n <n>', 'rerank 後の数', '5')
  .option('--no-rerank', 'リランク無効化')
  .option('--no-generate', 'LLM 応答生成を無効化（検索結果のみ）')
  .action(async (query: string, opts: { topK: string; topN: string; rerank: boolean; generate: boolean }) => {
    const docs = await retrieve(query, { topK: Number(opts.topK), topN: Number(opts.topN), rerank: opts.rerank });
    if (opts.generate) {
      const answer = await generate(query, docs);
      console.log('=== 回答 ===');
      console.log(answer);
    }
    console.log('\n=== 出典 ===');
    docs.forEach((d, i) => console.log(`[${i + 1}] ${d.source}${d.headings.length ? ' > ' + d.headings.join(' > ') : ''} (rerank=${d.rerankScore?.toFixed(3) ?? 'n/a'})`));
  });

program
  .command('status')
  .description('各サービスのヘルスと collection 統計')
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
  .description('collection を削除して再作成')
  .action(async () => {
    const client = getQdrantClient();
    await ensureCollection(client, true);
    console.log(JSON.stringify({ collection: config.QDRANT_COLLECTION, recreated: true }));
  });

program
  .command('serve')
  .description('Hono HTTP API を起動（127.0.0.1:7777）')
  .option('-p, --port <n>', 'ポート上書き')
  .action(async (opts: { port?: string }) => {
    if (opts.port) process.env.RAG_API_PORT = opts.port;
    await import('../api/server.js');
  });

program.parseAsync(process.argv).catch(e => { console.error(e); process.exit(1); });
```

### 13.2 検証

```bash
pnpm rag --help
# 期待: ingest / search / status / reindex / serve サブコマンドが表示

pnpm rag status
# 期待: 各サービスの ok/down と collection 一覧

pnpm rag ingest data/svg/hello.svg
pnpm rag ingest data/md/note.md            # Markdown
pnpm rag ingest data/txt/changelog.txt     # プレーンテキスト
pnpm rag ingest https://example.com         # 単一 URL
pnpm rag ingest data/url/refs.urls          # URL リスト（行ごと 1 URL）
pnpm rag ingest ./docs                     # ディレクトリ再帰（混在 OK）
pnpm rag search "RAG パイプラインの構成要素は？" --top-k 10 --top-n 3
pnpm rag reindex
```

---

## 14. Phase 10: 外部アプリ / クライアントからの利用例

本 RAG サブシステムは Hono REST API（`127.0.0.1:7777`）を公開しているため、curl / fetch / 任意言語の HTTP クライアントから自由に呼び出せる。本章は代表的なクライアント形態の例を示す。

### 14.1 シェルからの curl 呼び出し

#### 取込（`/ingest`）

```bash
# 単一ファイル
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./docs/spec.pdf"]}' | jq

# Markdown / テキスト
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./README.md","./CHANGELOG.txt"]}' | jq

# Web URL（単発 / 複数 / 混在）
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["https://qdrant.tech/documentation/","https://docs.docling-project.org/"]}' | jq

# ファイル + ディレクトリ + URL の混在
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./docs","./data/url/refs.urls","https://example.com"]}' | jq

# multipart アップロード
curl -fsS -X POST http://127.0.0.1:7777/ingest/upload \
  -F "file=@./design_rag.md" | jq
```

#### 検索（`/search`）

```bash
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"<クエリ>","top_k":20,"top_n":5}' | jq

# 出典のみ（LLM 生成スキップ）
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"<クエリ>","top_n":5,"generate":false}' | jq '.sources'
```

#### コレクション再構築（`/reindex`）

```bash
curl -fsS -X POST http://127.0.0.1:7777/reindex | jq
# 期待: {"collection":"rag_documents","recreated":true}
```

その後 `/ingest` を再実行して再投入する。

#### 状態確認（`/status`）

```bash
curl -fsS http://127.0.0.1:7777/status | jq
# collections[].points_count などで蓄積量を確認
```

### 14.2 Node.js / TypeScript クライアント例

```ts
// 任意の Node.js プロジェクトから fetch で呼ぶ最小例
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
console.log(await search('RAG パイプラインの構成は？'));
```

### 14.3 Python クライアント例

```python
import requests

RAG = 'http://127.0.0.1:7777'

# 取込
r = requests.post(f'{RAG}/ingest', json={'paths': ['./docs', 'https://example.com']})
print(r.json())

# 検索
r = requests.post(f'{RAG}/search', json={'query': 'RAG パイプラインの構成は？', 'top_k': 20, 'top_n': 5})
data = r.json()
print(data['answer'])
for s in data['sources']:
    print(f"- {s['source']} (score={s.get('rerankScore', s['score']):.3f})")
```

### 14.4 起動 / 停止 ランブック

```bash
# 起動
cd rag-system
docker compose up -d
pnpm serve &       # tsx で起動。pm2 や systemd でデーモン化推奨

# 停止
kill %1            # pnpm serve
docker compose stop

# 状態確認
curl -fsS http://127.0.0.1:7777/status | jq
```

`pnpm rag serve` を `~/.config/systemd/user/local-rag.service` に登録する例:

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

### 14.5 検証

API が外部から正しく利用できることを確認:

```bash
# 1. サーバが起動済
curl -fsS http://127.0.0.1:7777/health
# 期待: {"status":"ok"}

# 2. 取込と検索のラウンドトリップ
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["./README.md"]}' | jq
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"このプロジェクトの概要は？","top_n":3}' | jq
```

> **ネットワーク公開時の注意**: 既定では `127.0.0.1` バインドのため同一ホストからのみアクセス可能。LAN / 外部公開する場合は §16.3 セキュリティ留意点に従い、reverse proxy + 認証を必ず併用すること。

---

## 15. Phase 11: 運用・トラブルシューティング・評価

### 15.1 ログ

| 出口 | 場所 |
|-----|------|
| Hono | stdout（pino-pretty）、systemd 利用時は `journalctl --user -u rag_documents -f` |
| Qdrant | `docker logs rag-qdrant -f` |
| Ollama | `docker logs rag-ollama -f` |
| Docling Serve | `docker logs rag-docling -f` |

### 15.2 よくあるトラブル

| 症状 | 原因 | 対処 |
|-----|------|----|
| `/embeddings` で 404 | Ollama にモデル未投入 | `docker exec rag-ollama ollama pull bge-m3` |
| `Dim mismatch` | 違うモデルを pull した | `.env` の `OLLAMA_EMBED_MODEL` と `EMBED_DIM` を一致させる |
| Docling Serve OOM | 大量ページ PDF の同時投入 | `p-queue` の concurrency を 1 に下げる |
| transformers.js が重い | 初回モデル DL | 初回のみ。ローカルキャッシュ後は速くなる |
| ポート 7777 衝突 | 別プロセス使用中 | `RAG_API_PORT=7780 pnpm serve` |
| 外部クライアントから接続不可 | API がバインドしてない / firewall | `RAG_API_HOST=127.0.0.1` で同ホスト前提。別ホストなら 0.0.0.0 + reverse proxy |
| drawio で `pako.inflateRaw` 失敗 | 圧縮されていない平文 mxGraph | `decompressDiagram` 内の判定ガードを残しているので通常は問題なし |
| 日本語チャンクが極小 | `chunkSize` が小さすぎ | `.env` の `CHUNK_SIZE` を 1024 などに上げる |

### 15.3 評価

簡易評価セット（質問 + 期待出典）を `rag/eval/qa.jsonl` に置き、`pnpm rag search` を回してヒット率を測る:

```jsonl
{"q":"このプロジェクトの概要は何ですか？","expect_source":"README.md"}
{"q":"設定ファイルの配置先は？","expect_source":".env.example"}
```

```bash
# evaluator は別途実装（jq + while read で十分）。詳細は付録参照。
```

### 15.4 バックアップ

```bash
# Qdrant snapshot
curl -fsS -X POST http://127.0.0.1:6333/collections/rag_documents/snapshots
# → /qdrant/storage/snapshots に tar が生成される

# Docker volume バックアップ
docker run --rm -v rag-qdrant-data:/data -v $(pwd):/backup busybox \
  tar cvf /backup/qdrant-$(date +%F).tar /data
```

---

## 16. 付録

### 16.1 代替スタック

| 層 | 主 | 副 / 代替 |
|----|----|---------|
| ベクトル DB | Qdrant | Milvus / Weaviate（本ガイドはサポートしない、構成例のみ） |
| 埋め込み | Ollama bge-m3 | llama.cpp bge-m3 / `multilingual-e5-large` |
| LLM | Ollama qwen2.5 | llama.cpp qwen2.5 / `llama-3.1-8b-instruct` / `gemma-2-9b-it` |
| リランカ | bge-reranker-v2-m3 (ONNX) | `cross-encoder/ms-marco-MiniLM-L-12-v2` |
| ドキュメント変換 | Docling Serve | `unstructured.io` API（Python ベース、本ガイドはサポートしない） |
| API | Hono | Fastify / Express |

### 16.2 Sparse / ハイブリッド検索

参照ガイド `pdf_image_rag_guide_nodejs.md` の Phase 9 (BM25 ハイブリッド) を参照。`wink-bm25-text-search` + `kuromoji.js` で sparse vector を構築し、Qdrant の Multi-Vector で Dense と組合せる。本ガイドの収録対象外（必要に応じて拡張）。

### 16.3 セキュリティ留意点

- API は **`127.0.0.1` バインド固定**を既定とする。LAN / 外部公開する場合は別 reverse proxy（nginx / caddy）+ 認証必須。
- Docker Compose のポートも `127.0.0.1:` プレフィックスでループバックに限定済。
- `data/` 配下のドキュメントは社内秘扱い。`.gitignore` に追加し、Qdrant snapshot も同等の機密扱い。
- LLM プロンプトに credential を埋めない（出典 path のみで秘密値が引かれない設計）。
- transformers.js の HuggingFace モデル DL を遮断したい環境では事前に `~/.cache/huggingface/hub` に同期しておく。

### 16.4 性能チューニング

| パラメータ | 既定 | 上げる効果 | 下げる効果 |
|---------|------|----------|----------|
| `CHUNK_SIZE` | 512 | 文脈幅↑、再現↑ | 適合↑、検索精度↑ |
| `CHUNK_OVERLAP` | 64 | 境界欠落↓ | DB サイズ↓ |
| `TOP_K_RETRIEVE` | 20 | 再現↑（リランカ負荷↑） | レイテンシ↓ |
| `TOP_K_RERANK` | 5 | 文脈量↑ | LLM 入力↓ |
| `hnsw.m` | 16 | リコール↑ | メモリ↓ |
| `ef_construct` | 128 | 構築精度↑ | 構築速度↑ |
| Qdrant `on_disk` | false | RAM↓ | レイテンシ↑ |

### 16.5 参照ガイドからの転用箇所まとめ

| 本ガイド | 参照ガイド対応箇所 |
|---------|--------------------|
| §2 動作要件 | 参照ガイド §3 |
| §4 Phase 0 | 参照ガイド Phase 0 |
| §5 Phase 1 | 参照ガイド Phase 1 |
| §6.2 PDF 変換 | 参照ガイド Phase 2 (§2.2) |
| §7 チャンキング | 参照ガイド Phase 3 を日本語強化 |
| §8 埋め込み | 参照ガイド Phase 4 |
| §9 Qdrant | 参照ガイド Phase 5 |
| §10 検索 + リランク | 参照ガイド Phase 6 |
| §11 LLM 生成 | 参照ガイド Phase 7 |
| §12 Hono API | 参照ガイド Phase 8 を外部クライアント連携用にアレンジ |
| §16.2 Sparse / ハイブリッド | 参照ガイド Phase 9 |
| §15 運用 | 参照ガイド Phase 10 |

### 16.6 完了チェックリスト

- [ ] `rag-system/` 配下にプロジェクトを作成した
- [ ] `docker compose up -d` で Qdrant / Ollama / Docling Serve が起動する
- [ ] `bge-m3` / `qwen2.5:7b-instruct` を Ollama に投入した
- [ ] `pnpm rag ingest data/svg/<sample>.svg` が正常終了する
- [ ] `pnpm rag ingest data/drawio/<sample>.drawio` が正常終了する
- [ ] `pnpm rag ingest data/pdf/<sample>.pdf` が正常終了する
- [ ] `pnpm rag ingest data/md/<sample>.md` が正常終了する
- [ ] `pnpm rag ingest data/txt/<sample>.txt` が正常終了する
- [ ] `pnpm rag ingest https://example.com` が正常終了する
- [ ] `pnpm rag ingest data/url/refs.urls` で URL リストの一括取込が正常終了する
- [ ] `pnpm rag search "<クエリ>"` が回答 + 出典を返す
- [ ] `curl http://127.0.0.1:7777/health` が `{"status":"ok"}` を返す
- [ ] 外部クライアント（curl / fetch / Python など）から `/ingest` `/search` を呼び出せる
- [ ] `RAG_BACKEND=llamacpp` で llama.cpp 経路に切替えられる（任意）

---

**ガイド終わり** — 構築上の不明点や追加要件は `.aiprj/instructions.md` 経由で次セッションへ持ち込んでください。
