import { readFile } from 'node:fs/promises';
import { XMLParser } from 'fast-xml-parser';
import { basename } from 'node:path';
import type { ConvertedDoc } from './index.js';

interface SvgTextElement {
  text: string;
  x?: number;
  y?: number;
  kind: 'text' | 'tspan' | 'title' | 'desc';
}

const TEXT_TAGS = new Set(['text', 'tspan', 'title', 'desc']);

function walk(node: unknown, acc: SvgTextElement[]): void {
  if (node == null || typeof node !== 'object') return;
  for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
    if (Array.isArray(value)) {
      for (const v of value) walkTagged(key, v, acc);
    } else if (typeof value === 'object') {
      walkTagged(key, value, acc);
    }
  }
}

function walkTagged(tag: string, node: unknown, acc: SvgTextElement[]): void {
  if (node == null || typeof node !== 'object') return;
  const obj = node as Record<string, unknown>;
  if (TEXT_TAGS.has(tag)) {
    const text = collectText(obj);
    if (text.trim()) {
      acc.push({
        kind: tag as SvgTextElement['kind'],
        x: typeof obj.x === 'string' ? Number(obj.x) : (obj.x as number | undefined),
        y: typeof obj.y === 'string' ? Number(obj.y) : (obj.y as number | undefined),
        text: text.trim(),
      });
    }
  }
  walk(obj, acc);
}

function collectText(obj: Record<string, unknown>): string {
  if (typeof obj['#text'] === 'string') return obj['#text'];
  let s = '';
  for (const [k, v] of Object.entries(obj)) {
    if (k === '#text' && typeof v === 'string') s += v;
    if (Array.isArray(v)) for (const vi of v) if (vi && typeof vi === 'object') s += collectText(vi as Record<string, unknown>);
    if (v && typeof v === 'object' && !Array.isArray(v)) s += collectText(v as Record<string, unknown>);
  }
  return s;
}

export async function convertSvg(filePath: string): Promise<ConvertedDoc> {
  const xml = await readFile(filePath, 'utf-8');
  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: '',
    preserveOrder: false,
    allowBooleanAttributes: true,
  });
  const root = parser.parse(xml);
  const acc: SvgTextElement[] = [];
  walk(root, acc);

  // .drawio.svg は <svg content="..."> に mxGraph XML が埋め込まれている場合あり
  // → drawio.ts と共用するためここでも展開を試みる
  const svgRoot = (root as Record<string, unknown>).svg as Record<string, unknown> | undefined;
  const embeddedContent = svgRoot?.content;
  let drawioMd = '';
  if (typeof embeddedContent === 'string' && embeddedContent.trim().startsWith('<')) {
    const { extractMxCells } = await import('./drawio.js');
    drawioMd = extractMxCells(embeddedContent);
  }

  const lines: string[] = [`# SVG: ${basename(filePath)}`, ''];
  for (const el of acc) {
    const pos = el.x != null && el.y != null ? ` (x=${el.x}, y=${el.y})` : '';
    lines.push(`- [${el.kind}]${pos} ${el.text}`);
  }
  if (drawioMd) {
    lines.push('', '## Embedded drawio cells', '', drawioMd);
  }
  return {
    source: filePath,
    markdown: lines.join('\n'),
    metadata: { kind: 'svg', elementCount: acc.length },
  };
}
