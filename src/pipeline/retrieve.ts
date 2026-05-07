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

  const candidates: RetrievedDoc[] = hits.map(h => ({
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
