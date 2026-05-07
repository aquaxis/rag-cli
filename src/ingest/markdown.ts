import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import matter from 'gray-matter';
import type { ConvertedDoc } from './index.js';

/**
 * Markdown はそのまま投入する。frontmatter は metadata に分離。
 */
export async function convertMarkdown(filePath: string): Promise<ConvertedDoc> {
  const raw = await readFile(filePath, 'utf-8');
  const { content, data: frontmatter } = matter(raw);
  return {
    source: filePath,
    markdown: content,
    metadata: {
      kind: 'markdown',
      frontmatter: Object.keys(frontmatter).length ? frontmatter : undefined,
      title: typeof frontmatter.title === 'string' ? frontmatter.title : basename(filePath),
    },
  };
}

/**
 * プレーンテキスト / .log / .rst は最低限の整形だけ行い Markdown 同等に扱う。
 * - BOM 除去
 * - CRLF → LF
 * - 連続空行を 1 行に圧縮（過剰チャンク化防止）
 */
export async function convertText(filePath: string): Promise<ConvertedDoc> {
  let raw = await readFile(filePath, 'utf-8');
  if (raw.charCodeAt(0) === 0xfeff) raw = raw.slice(1);
  raw = raw.replace(/\r\n/g, '\n').replace(/\n{3,}/g, '\n\n');
  return {
    source: filePath,
    markdown: `# ${basename(filePath)}\n\n${raw}`,
    metadata: { kind: 'text' },
  };
}
