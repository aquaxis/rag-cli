import 'dotenv/config';
import { z } from 'zod';

const envSchema = z.object({
  QDRANT_URL: z.string().url().default('http://127.0.0.1:6333'),
  QDRANT_API_KEY: z.string().optional(),
  QDRANT_COLLECTION: z.string().default('rag_documents'),

  RAG_BACKEND: z.enum(['ollama', 'llamacpp']).default('ollama'),

  OLLAMA_HOST: z.string().url().default('http://127.0.0.1:11434'),
  OLLAMA_LLM_MODEL: z.string().default('qwen2.5:7b-instruct'),
  OLLAMA_EMBED_MODEL: z.string().default('bge-m3'),

  LLAMACPP_EMBED_URL: z.string().url().default('http://127.0.0.1:8080/v1'),
  LLAMACPP_LLM_URL: z.string().url().default('http://127.0.0.1:8081/v1'),
  LLAMACPP_EMBED_MODEL: z.string().default('bge-m3'),
  LLAMACPP_LLM_MODEL: z.string().default('qwen2.5-7b-instruct'),

  DOCLING_URL: z.string().url().default('http://127.0.0.1:5001'),
  RERANKER_MODEL: z.string().default('onnx-community/bge-reranker-v2-m3-ONNX'),

  RAG_API_HOST: z.string().default('127.0.0.1'),
  RAG_API_PORT: z.coerce.number().default(7777),

  CHUNK_SIZE: z.coerce.number().default(512),
  CHUNK_OVERLAP: z.coerce.number().default(64),
  TOP_K_RETRIEVE: z.coerce.number().default(20),
  TOP_K_RERANK: z.coerce.number().default(5),
  EMBED_DIM: z.coerce.number().default(1024),

  LOG_LEVEL: z.enum(['trace', 'debug', 'info', 'warn', 'error']).default('info'),
});

export const config = envSchema.parse(process.env);
export type Config = typeof config;
