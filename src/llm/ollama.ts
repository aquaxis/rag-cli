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
