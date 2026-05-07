import { config } from '../config.js';
import { generateOllama, streamOllama } from './ollama.js';
import { generateLlamaCpp, streamLlamaCpp } from './llamacpp.js';
import type { RetrievedDoc } from '../pipeline/retrieve.js';

const SYSTEM_PROMPT = `あなたは提供されたドキュメントに基づいて正確に回答するアシスタントです。
以下を厳守:
1. 「参考情報」のみに基づいて回答する。想像で補わない。
2. 答えがない場合は「提供された情報では回答できません」と明言する。
3. 末尾に [1][2] 形式で出典番号を列挙する。
4. 数値・日付は原文どおりに引用する。`;

export function buildUserMessage(question: string, docs: RetrievedDoc[]): string {
  const ctx = docs.map((d, i) => {
    const path = [d.source, ...d.headings].join(' > ');
    return `[${i + 1}] 出典: ${path}\n${d.text}`;
  }).join('\n\n---\n\n');
  return `【質問】\n${question}\n\n【参考情報】\n${ctx}\n\n上記に基づいて回答してください。`;
}

export async function generate(question: string, docs: RetrievedDoc[]): Promise<string> {
  const fn = config.RAG_BACKEND === 'llamacpp' ? generateLlamaCpp : generateOllama;
  return fn(SYSTEM_PROMPT, buildUserMessage(question, docs));
}

export function generateStream(question: string, docs: RetrievedDoc[]): AsyncGenerator<string> {
  const fn = config.RAG_BACKEND === 'llamacpp' ? streamLlamaCpp : streamOllama;
  return fn(SYSTEM_PROMPT, buildUserMessage(question, docs));
}
