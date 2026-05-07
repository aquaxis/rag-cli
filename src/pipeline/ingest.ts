import { randomUUID } from 'node:crypto';
import PQueue from 'p-queue';
import { glob } from 'node:fs/promises';
import { join } from 'node:path';
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
