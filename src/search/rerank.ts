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
