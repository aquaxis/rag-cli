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
