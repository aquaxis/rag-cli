# 04. REST API リファレンス

`rag-cli serve` で起動する Rust 製 REST API。既定で `127.0.0.1:7777` にバインドする。一次ソースは [`crates/api/src/lib.rs`](../crates/api/src/lib.rs)。

## 共通仕様

- ベース URL: `http://127.0.0.1:7777`（`RAG_API_HOST` `RAG_API_PORT` で上書き）
- ミドルウェア: CORS（permissive）、リクエストロガー、15 分タイムアウト
- リクエスト / レスポンスは UTF-8 JSON
- エラー時は HTTP 500（または 400 / 404）+ `{"error":"<message>"}`
- リクエストボディの上限: 100 MB（`/ingest/upload` を含む）

## エンドポイント一覧

| メソッド | パス | 用途 |
|---------|------|------|
| GET | `/health` | 死活確認 |
| GET | `/status` | サービスヘルス + collection 一覧 |
| POST | `/ingest` | パス / URL 配列を取込 |
| POST | `/ingest/upload` | multipart ファイルアップロード取込 |
| POST | `/search` | 検索 + LLM 応答 |
| POST | `/search/stream` | 出典先送り + LLM トークンストリーム |
| POST | `/reindex` | collection 削除 + 再作成 |

## `GET /health`

死活確認。

レスポンス:

```json
{"status":"ok"}
```

```bash
curl -fsS http://127.0.0.1:7777/health
```

## `GET /status`

Qdrant / Ollama / Docling Serve のヘルスチェックと Qdrant collection 一覧。

レスポンス例:

```json
{
  "qdrant": "ok",
  "ollama": "ok",
  "docling": "down",
  "backend": "ollama",
  "collections": [
    { "name": "rag_documents" }
  ]
}
```

```bash
curl -fsS http://127.0.0.1:7777/status | jq
```

## `POST /ingest`

ローカルパス / URL を混在配列で取込む。

リクエスト:

```json
{
  "paths": ["data/md/note.md", "https://example.com", "data/url/refs.urls"],
  "collection": "rag_documents"
}
```

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| `paths` | `string[]` | ✅ | 1 件以上。ローカルパス / ディレクトリ / URL / `.urls` の混在可 |
| `collection` | `string` | | （現バージョンでは無視。将来拡張用） |

レスポンス:

```json
{
  "ingested": 1,
  "chunks": 1,
  "errors": 0,
  "total": 1
}
```

| フィールド | 説明 |
|-----------|------|
| `ingested` | 成功した取込件数 |
| `chunks` | 投入された chunk の合計数 |
| `errors` | 失敗件数（`stderr` にログ出力） |
| `total` | `paths` を展開した後の総件数（ディレクトリ再帰や `.urls` 展開を含む） |

```bash
curl -fsS -X POST http://127.0.0.1:7777/ingest \
  -H 'content-type: application/json' \
  -d '{"paths":["data/md/note.md"]}'
```

## `POST /ingest/upload`

multipart ファイルアップロードで取込む。`data/upload/<original_filename>` に保存後、通常の取込パイプラインに流す。

リクエスト: `multipart/form-data` の `file` フィールド（必須）。

レスポンス:

```json
{"path":"data/upload/note.md","chunks":1}
```

```bash
curl -fsS -X POST http://127.0.0.1:7777/ingest/upload \
  -F "file=@data/md/note.md"
```

## `POST /search`

検索 + LLM 応答（オプショナル）。

リクエスト:

```json
{
  "query": "サンプルメモの概要は？",
  "top_k": 20,
  "top_n": 3,
  "rerank": true,
  "generate": true
}
```

| フィールド | 型 | 既定 | 説明 |
|-----------|-----|------|------|
| `query` | `string` | | 1〜2000 文字（必須） |
| `top_k` | `number` | `TOP_K_RETRIEVE`（20） | retrieve 候補数 |
| `top_n` | `number` | `TOP_K_RERANK`（5） | 出力数 |
| `rerank` | `boolean` | `true` | `false` で rerank をスキップ |
| `generate` | `boolean` | `true` | `false` で LLM 応答生成を省略 |

レスポンス:

```json
{
  "answer": "本書はサンプルメモであり... [1][2]",
  "sources": [
    {
      "id": "uuid",
      "text": "...",
      "score": 0.6516,
      "rerankScore": 2.146,
      "source": "data/md/note.md",
      "chunkId": 0,
      "headings": ["概要"],
      "kind": "file"
    }
  ]
}
```

`generate=false` のとき `answer` は `null`。`rerank=false` のとき `rerankScore` は省略される。

```bash
curl -fsS -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"サンプルメモ","top_n":3,"rerank":false,"generate":false}'
```

## `POST /search/stream`

検索結果と LLM トークンを chunked HTTP body でストリーム配信する。リクエストフィールドは `/search` と同一。

レスポンスフォーマット:

```text
{"type":"sources","sources":[...]}\n
---\n
<LLM トークン>...
```

1 行目は `sources` を含む JSON、2 行目は区切り `---`、3 行目以降は LLM の生成トークン。

```bash
curl -N -X POST http://127.0.0.1:7777/search/stream \
  -H 'content-type: application/json' \
  -d '{"query":"サンプルメモ","top_n":3}'
```

クライアント側は最初の `\n` までを `sources` として、`---\n` の後を `answer` として読み出す。

## `POST /reindex`

Qdrant collection を削除して再作成する。

リクエスト: ボディなし。

レスポンス:

```json
{"collection":"rag_documents","recreated":true}
```

```bash
curl -fsS -X POST http://127.0.0.1:7777/reindex
```

## エラーレスポンス

| HTTP | 形式 | 例 |
|------|------|----|
| 400 | `{"error":"<msg>"}` | バリデーション失敗（`paths` 空、`query` 範囲外など） |
| 404 | `{"error":"<msg>"}` | 取込対象パスが存在しない |
| 500 | `{"error":"<msg>"}` | Qdrant / Ollama / Docling 通信失敗、ort 推論失敗など |

---

← [`./03-cli.md`](./03-cli.md) | → [`./05-configuration.md`](./05-configuration.md)
