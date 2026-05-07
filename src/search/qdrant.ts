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
