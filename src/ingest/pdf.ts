import { readFile } from 'node:fs/promises';
import { basename } from 'node:path';
import { config } from '../config.js';
import { logger } from '../logger.js';
import type { ConvertedDoc } from './index.js';

export async function convertPdf(filePath: string): Promise<ConvertedDoc> {
  const data = await readFile(filePath);
  const blob = new Blob([data]);
  const form = new FormData();
  form.append('files', blob, basename(filePath));
  form.append('parameters', JSON.stringify({
    from_formats: ['pdf', 'image', 'docx', 'pptx', 'html', 'md'],
    to_formats: ['md'],
    do_ocr: true,
    ocr_engine: 'easyocr',
    ocr_lang: ['ja', 'en'],
    table_mode: 'accurate',
    image_export_mode: 'placeholder',
    abort_on_error: false,
    return_as_file: false,
  }));

  const url = `${config.DOCLING_URL}/v1alpha/convert/file`;
  logger.info({ filePath }, 'Calling Docling Serve');
  const res = await fetch(url, { method: 'POST', body: form, signal: AbortSignal.timeout(10 * 60 * 1000) });
  if (!res.ok) throw new Error(`Docling failed (${res.status}): ${await res.text()}`);

  const json = await res.json() as { document?: { md_content?: string }; status?: string; errors?: Array<{ error_message: string }> };
  if (json.status !== 'success' && json.errors?.length) {
    throw new Error(`Docling errors: ${json.errors.map(e => e.error_message).join('; ')}`);
  }
  const md = json.document?.md_content ?? '';
  if (!md) throw new Error('Empty markdown from Docling');
  return { source: filePath, markdown: md, metadata: { kind: 'pdf' } };
}
