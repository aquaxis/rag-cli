import { extname } from 'node:path';
import { convertPdf } from './pdf.js';
import { convertSvg } from './svg.js';
import { convertDrawio } from './drawio.js';
import { convertMarkdown, convertText } from './markdown.js';
import { convertWeb } from './web.js';

export interface ConvertedDoc {
  source: string;
  markdown: string;
  metadata: Record<string, unknown>;
}

export function isUrl(input: string): boolean {
  return /^https?:\/\//i.test(input);
}

export async function convertAny(input: string): Promise<ConvertedDoc> {
  if (isUrl(input)) return convertWeb(input);
  const ext = extname(input).toLowerCase();
  if (ext === '.pdf' || ['.png', '.jpg', '.jpeg', '.tiff', '.bmp'].includes(ext)) {
    return convertPdf(input);
  }
  if (ext === '.svg' || input.endsWith('.drawio.svg')) {
    return convertSvg(input);
  }
  if (ext === '.drawio') {
    return convertDrawio(input);
  }
  if (ext === '.md' || ext === '.markdown') {
    return convertMarkdown(input);
  }
  if (ext === '.txt' || ext === '.log' || ext === '.rst') {
    return convertText(input);
  }
  throw new Error(`Unsupported format: ${input}`);
}
