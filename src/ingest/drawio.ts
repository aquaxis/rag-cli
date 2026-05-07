import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { XMLParser } from 'fast-xml-parser';
import pako from 'pako';
import type { ConvertedDoc } from './index.js';

/**
 * drawio の <diagram> 要素の中身は次のいずれか:
 *   1) 平文 mxGraph XML (<mxGraphModel>...</mxGraphModel>)
 *   2) deflate + base64 でエンコードされた URL-encoded mxGraph XML
 */
export function decompressDiagram(content: string): string {
  const trimmed = content.trim();
  if (trimmed.startsWith('<mxGraphModel') || trimmed.startsWith('<?xml')) {
    return trimmed;
  }
  // base64 → bytes → inflateRaw → URL decode
  const buf = Buffer.from(trimmed, 'base64');
  const inflated = pako.inflateRaw(buf, { to: 'string' });
  return decodeURIComponent(inflated);
}

interface MxCellLabel { id?: string; label: string; parent?: string; }

export function extractMxCells(mxXml: string): string {
  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: '',
    allowBooleanAttributes: true,
  });
  const root = parser.parse(mxXml);
  const cells: MxCellLabel[] = [];
  collectCells(root, cells);
  return cells.map(c => `- ${c.label}${c.id ? ` (id=${c.id})` : ''}`).join('\n');
}

function collectCells(node: unknown, acc: MxCellLabel[]): void {
  if (!node || typeof node !== 'object') return;
  const obj = node as Record<string, unknown>;
  for (const [key, value] of Object.entries(obj)) {
    const items = Array.isArray(value) ? value : [value];
    for (const item of items) {
      if (!item || typeof item !== 'object') continue;
      const it = item as Record<string, unknown>;
      if (key === 'mxCell' || key === 'UserObject') {
        const label = (it.value ?? it.label) as string | undefined;
        if (typeof label === 'string' && label.trim()) {
          acc.push({
            id: typeof it.id === 'string' ? it.id : undefined,
            label: stripHtml(label).trim(),
          });
        }
      }
      collectCells(it, acc);
    }
  }
}

function stripHtml(s: string): string {
  return s.replace(/<[^>]+>/g, ' ').replace(/&nbsp;/g, ' ').replace(/\s+/g, ' ');
}

export async function convertDrawio(filePath: string): Promise<ConvertedDoc> {
  const xml = await readFile(filePath, 'utf-8');
  const parser = new XMLParser({ ignoreAttributes: false, attributeNamePrefix: '' });
  const root = parser.parse(xml);

  // 多段ネスト: mxfile > diagram[*] > (compressed | mxGraphModel)
  const diagrams = collectDiagrams(root);
  const lines: string[] = [`# drawio: ${basename(filePath)}`, ''];
  for (const [i, d] of diagrams.entries()) {
    const inner = decompressDiagram(typeof d === 'string' ? d : (d as { '#text'?: string })['#text'] ?? '');
    const name = typeof d === 'object' && d !== null && (d as Record<string, unknown>).name
      ? `: ${(d as Record<string, unknown>).name}`
      : '';
    lines.push(`## Diagram ${i + 1}${name}`, '', extractMxCells(inner), '');
  }
  return {
    source: filePath,
    markdown: lines.join('\n'),
    metadata: { kind: 'drawio', diagramCount: diagrams.length },
  };
}

function collectDiagrams(node: unknown): unknown[] {
  if (!node || typeof node !== 'object') return [];
  const obj = node as Record<string, unknown>;
  if (Array.isArray(obj.diagram)) return obj.diagram;
  if (obj.diagram) return [obj.diagram];
  for (const v of Object.values(obj)) {
    if (v && typeof v === 'object') {
      const r = collectDiagrams(v);
      if (r.length) return r;
    }
  }
  return [];
}
