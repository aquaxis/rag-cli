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
