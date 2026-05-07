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
