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
