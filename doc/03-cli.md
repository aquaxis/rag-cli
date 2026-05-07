# 03. CLI リファレンス

`rag-cli` は `clap` ベースのサブコマンド型 CLI。一次ソースは [`crates/cli/src/main.rs`](../crates/cli/src/main.rs)。

## 全体

```text
rag-cli <COMMAND>

Commands:
  ingest   ファイル / ディレクトリ / URL / .urls を取込
  search   検索 + LLM 応答
  status   各サービスのヘルスと collection
  reindex  collection を削除して再作成
  serve    Hono 互換 HTTP API を起動
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

すべてのサブコマンドは `--help` で詳細を表示する。

## `ingest`

```text
rag-cli ingest <TARGET>
```

| 引数 | 必須 | 説明 |
|------|------|------|
| `<TARGET>` | ✅ | ファイル / ディレクトリ / URL / `.urls` ファイル パス |

入力種別の判定:

- `http://` `https://` で始まれば URL として扱い Docling Serve に転送。
- `.urls` 拡張子のファイルは行ごとに URL として展開（`#` でコメント、空行除外）。
- ディレクトリは再帰的に展開され、対応拡張子（`.pdf` `.png` `.jpg` `.jpeg` `.tiff` `.bmp` `.svg` `.drawio` `.md` `.markdown` `.txt` `.log` `.rst` `.urls`）のみを取込む。
- 単一ファイルはその拡張子に応じて変換器を選択する。

出力（単一ファイル）:

```json
{"chunks":1,"source":"data/md/note.md"}
```

出力（複数ファイル / ディレクトリ）:

```json
{
  "ingested": 4,
  "chunks": 4,
  "errors": 0,
  "total": 6
}
```

並列度は `tokio::sync::Semaphore::new(2)` で固定（同時 2 件まで）。

## `search`

```text
rag-cli search <QUERY> [OPTIONS]
```

| 引数 | デフォルト | 説明 |
|------|------------|------|
| `<QUERY>` |  | 検索クエリ（必須、1〜2000 文字） |
| `-k, --top-k <N>` | `20` | retrieve 候補数（Qdrant Dense 検索の上限） |
| `-n, --top-n <N>` | `5` | rerank 後の出力数 |
| `--no-rerank` | off | リランクをスキップ（bi-encoder のスコアのみで上位を返す） |
| `--no-generate` | off | LLM 応答生成を無効化（出典のみ表示） |

出力:

```text
=== 回答 ===
（LLM 応答、--no-generate 指定時はこの章ごと省略）

=== 出典 ===
[1] <source> > <h1> > <h2> (rerank=<score>)
[2] ...
```

`rerank=` は rerank を行ったときのスコア。`--no-rerank` 指定時は `n/a`。

## `status`

```text
rag-cli status
```

引数なし。Qdrant / Ollama / Docling Serve のヘルスチェックと、Qdrant の collection 一覧を JSON で返す。

```json
{
  "backend": "ollama",
  "collections": [
    { "name": "rag_documents" }
  ],
  "docling": "down",
  "ollama": "ok",
  "qdrant": "ok"
}
```

`backend` の値は `RAG_BACKEND` の現在値（`ollama` または `llamacpp`）。

## `reindex`

```text
rag-cli reindex
```

引数なし。Qdrant の `rag_documents`（`QDRANT_COLLECTION` で上書き可）collection を削除して再作成する。

```json
{"collection":"rag_documents","recreated":true}
```

取込済データはすべて消えるため、再投入が必要。

## `serve`

```text
rag-cli serve [OPTIONS]
```

| 引数 | デフォルト | 説明 |
|------|------------|------|
| `-p, --port <N>` | `RAG_API_PORT`（既定 7777） | API バインドポート |

`RAG_API_HOST`（既定 `127.0.0.1`）で listen し、Ctrl-C / SIGINT で停止する。エンドポイントは [`./04-rest-api.md`](./04-rest-api.md) を参照。

```bash
rag-cli serve --port 7780
# → 127.0.0.1:7780 で起動
```

## 終了コード

| コード | 意味 |
|-------|------|
| `0` | 成功 |
| `1` | 失敗（取込・検索・API 起動エラーなど。`stderr` に `Error: <message>` を出力） |

## ログ

`tracing` を使ったログを `stderr` に出力する。レベルは `LOG_LEVEL` 環境変数（既定 `info`）で制御。

```bash
LOG_LEVEL=debug rag-cli search "..."
```

詳細は [`./05-configuration.md`](./05-configuration.md) を参照。

---

← [`./02-quickstart.md`](./02-quickstart.md) | → [`./04-rest-api.md`](./04-rest-api.md)
