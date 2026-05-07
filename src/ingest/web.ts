import { config } from '../config.js';
import { logger } from '../logger.js';
import type { ConvertedDoc } from './index.js';

interface DoclingSourceResponse {
  document?: { md_content?: string; filename?: string };
  status?: string;
  errors?: Array<{ error_message: string }>;
}

/**
 * URL を Docling Serve に渡し Markdown を取得する。
 * - HTTP / HTTPS のみ許可
 * - HTML / PDF / 画像のいずれでも 1 経路で扱える
 * - OCR は HTML では発火しないが、PDF / 画像 URL では自動で発火
 */
export async function convertWeb(url: string): Promise<ConvertedDoc> {
  if (!/^https?:\/\//i.test(url)) throw new Error(`Not an HTTP(S) URL: ${url}`);

  const body = {
    sources: [{ kind: 'http', url }],
    options: {
      from_formats: ['pdf', 'image', 'docx', 'pptx', 'html', 'md'],
      to_formats: ['md'],
      do_ocr: true,
      ocr_engine: 'easyocr',
      ocr_lang: ['ja', 'en'],
      table_mode: 'accurate',
      image_export_mode: 'placeholder',
      abort_on_error: false,
      return_as_file: false,
    },
  };

  logger.info({ url }, 'Calling Docling Serve (URL source)');
  const res = await fetch(`${config.DOCLING_URL}/v1alpha/convert/source`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(10 * 60 * 1000),
  });
  if (!res.ok) throw new Error(`Docling URL convert failed (${res.status}): ${await res.text()}`);

  const json = await res.json() as DoclingSourceResponse;
  if (json.status !== 'success' && json.errors?.length) {
    throw new Error(`Docling errors: ${json.errors.map(e => e.error_message).join('; ')}`);
  }
  const md = json.document?.md_content ?? '';
  if (!md) throw new Error(`Empty markdown from URL: ${url}`);

  return {
    source: url,
    markdown: md,
    metadata: {
      kind: 'web',
      url,
      filename: json.document?.filename,
      fetchedAt: new Date().toISOString(),
    },
  };
}

/**
 * `.urls` 拡張子のテキストファイル（行ごと 1 URL、`#` で始まる行はコメント）を読み URL 配列を返す。
 * `expandPath` で展開され、各 URL は `convertWeb` で個別取込される。
 */
export async function readUrlList(filePath: string): Promise<string[]> {
  const { readFile } = await import('node:fs/promises');
  const raw = await readFile(filePath, 'utf-8');
  return raw.split(/\r?\n/).map(l => l.trim()).filter(l => l && !l.startsWith('#'));
}
