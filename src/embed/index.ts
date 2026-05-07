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
