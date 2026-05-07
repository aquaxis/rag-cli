import {
  MarkdownTextSplitter,
  RecursiveCharacterTextSplitter,
} from '@langchain/textsplitters';
import matter from 'gray-matter';
import { config } from '../config.js';

export interface Chunk {
  text: string;
  metadata: {
    source: string;
    chunkId: number;
    headings: string[];
    frontmatter?: Record<string, unknown>;
  };
}

const JP_SEPARATORS = [
  '\n\n',  // パラグラフ
  '\n',    // 行
  '。', '！', '？',  // 日本語句点
  '. ', '! ', '? ', // 英文末
  '、',                // 日本語読点（最終手段）
  ' ',                 // 半角空白
  '',                  // 文字単位
];

export async function chunkJapanese(
  source: string,
  markdown: string,
): Promise<Chunk[]> {
  const { content, data: frontmatter } = matter(markdown);

  // 見出し構造で粗く分割
  const mdSplitter = new MarkdownTextSplitter({
    chunkSize: config.CHUNK_SIZE * 4,
    chunkOverlap: 0,
  });
  const sections = await mdSplitter.splitText(content);

  // 日本語句読点優先で細粒度分割
  const refine = new RecursiveCharacterTextSplitter({
    chunkSize: config.CHUNK_SIZE * 3,        // 1 トークン ≒ 2-3 文字（bge-m3 日本語）
    chunkOverlap: config.CHUNK_OVERLAP * 3,
    separators: JP_SEPARATORS,
  });

  const chunks: Chunk[] = [];
  let idx = 0;

  for (const section of sections) {
    const headings = extractHeadings(section);
    const parts = await refine.splitText(section);
    for (const body of parts) {
      const text = contextualize(headings, body);
      if (text.trim().length < 8) continue; // 過小チャンク除外
      chunks.push({
        text,
        metadata: {
          source,
          chunkId: idx++,
          headings,
          frontmatter: Object.keys(frontmatter).length ? frontmatter : undefined,
        },
      });
    }
  }
  return chunks;
}

function extractHeadings(md: string): string[] {
  const out: string[] = [];
  for (const line of md.split('\n')) {
    const m = /^(#{1,6})\s+(.+?)\s*$/.exec(line);
    if (m && m[2]) out.push(m[2]);
  }
  return out;
}

function contextualize(headings: string[], body: string): string {
  if (headings.length === 0) return body;
  return `${headings.map(h => `# ${h}`).join(' > ')}\n\n${body}`;
}
