import { Command } from 'commander';
import { ingestPath, ingestPaths, expandPath } from '../pipeline/ingest.js';
import { retrieve } from '../pipeline/retrieve.js';
import { generate } from '../llm/index.js';
import { ensureCollection, getQdrantClient } from '../search/qdrant.js';
import { config } from '../config.js';

const program = new Command();
program.name('local-rag').description('スタンドアロン RAG CLI').version('0.1.0');

program
  .command('ingest <target>')
  .description('ファイル / ディレクトリ / URL / .urls ファイル を取込（混在可）')
  .action(async (target: string) => {
    const expanded = await expandPath(target);
    if (expanded.length === 1) {
      const r = await ingestPath(expanded[0]!);
      console.log(JSON.stringify({ source: expanded[0], ...r }));
    } else {
      const stats = await ingestPaths(expanded);
      console.log(JSON.stringify({ ...stats, total: expanded.length }, null, 2));
    }
  });

program
  .command('search <query>')
  .description('検索 + LLM 応答')
  .option('-k, --top-k <n>', 'retrieve 候補数', '20')
  .option('-n, --top-n <n>', 'rerank 後の数', '5')
  .option('--no-rerank', 'リランク無効化')
  .option('--no-generate', 'LLM 応答生成を無効化（検索結果のみ）')
  .action(async (query: string, opts: { topK: string; topN: string; rerank: boolean; generate: boolean }) => {
    const docs = await retrieve(query, { topK: Number(opts.topK), topN: Number(opts.topN), rerank: opts.rerank });
    if (opts.generate) {
      const answer = await generate(query, docs);
      console.log('=== 回答 ===');
      console.log(answer);
    }
    console.log('\n=== 出典 ===');
    docs.forEach((d, i) => console.log(`[${i + 1}] ${d.source}${d.headings.length ? ' > ' + d.headings.join(' > ') : ''} (rerank=${d.rerankScore?.toFixed(3) ?? 'n/a'})`));
  });

program
  .command('status')
  .description('各サービスのヘルスと collection 統計')
  .action(async () => {
    const probes = await Promise.allSettled([
      fetch(`${config.QDRANT_URL}/readyz`),
      fetch(`${config.OLLAMA_HOST}/api/tags`),
      fetch(`${config.DOCLING_URL}/health`),
    ]);
    const client = getQdrantClient();
    const collections = await client.getCollections().then(r => r.collections).catch(() => []);
    console.log(JSON.stringify({
      qdrant: probes[0].status === 'fulfilled' && probes[0].value.ok ? 'ok' : 'down',
      ollama: probes[1].status === 'fulfilled' && probes[1].value.ok ? 'ok' : 'down',
      docling: probes[2].status === 'fulfilled' && probes[2].value.ok ? 'ok' : 'down',
      backend: config.RAG_BACKEND,
      collections,
    }, null, 2));
  });

program
  .command('reindex')
  .description('collection を削除して再作成')
  .action(async () => {
    const client = getQdrantClient();
    await ensureCollection(client, true);
    console.log(JSON.stringify({ collection: config.QDRANT_COLLECTION, recreated: true }));
  });

program
  .command('serve')
  .description('Hono HTTP API を起動（127.0.0.1:7777）')
  .option('-p, --port <n>', 'ポート上書き')
  .action(async (opts: { port?: string }) => {
    if (opts.port) process.env.RAG_API_PORT = opts.port;
    await import('../api/server.js');
  });

program.parseAsync(process.argv).catch(e => { console.error(e); process.exit(1); });
