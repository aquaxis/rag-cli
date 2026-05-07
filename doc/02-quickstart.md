# 02. クイックスタート

最短ルートで取込・検索・API 起動まで動かす手順。詳細は [`./03-cli.md`](./03-cli.md) や [`./04-rest-api.md`](./04-rest-api.md) を参照。

## 前提

[`./01-installation.md`](./01-installation.md) のインストールが完了し、以下が起動していること:

- Qdrant（`127.0.0.1:6333`）
- Ollama（`127.0.0.1:11434`）+ `bge-m3` + `qwen2.5:7b-instruct`
- （任意）Docling Serve（`127.0.0.1:5001`、PDF / 画像 / Web URL 取込用）

```bash
./target/release/rag-cli status
# → qdrant: ok / ollama: ok / docling: ok（Docling 起動時）
```

## 5 ステップ

### 1. サンプルデータを取込む

```bash
./target/release/rag-cli ingest data/md/note.md
# → {"chunks":1,"source":"data/md/note.md"}
```

ディレクトリ単位の取込もできる:

```bash
./target/release/rag-cli ingest data/
# → {"ingested":4,"chunks":4,"errors":0,"total":6}
```

### 2. 検索（リランクなし、最速）

```bash
./target/release/rag-cli search "サンプルメモの概要は？" --top-n 3 --no-rerank
```

出力例:

```text
=== 回答 ===
本書はサンプルメモであり、日本語ドキュメントを取り込み、検索とリランクで上位の結果を返します。[1][2][3]

=== 出典 ===
[1] data/md/note.md > 概要 (rerank=n/a)
[2] data/txt/changelog.txt > changelog.txt (rerank=n/a)
[3] data/svg/hello.svg > SVG: hello.svg (rerank=n/a)
```

### 3. 検索（リランクあり、精度優先）

```bash
./target/release/rag-cli search "サンプルメモの概要は？" --top-n 3
```

初回は HuggingFace Hub から bge-reranker-v2-m3-ONNX（約 600 MB）を DL するため数十秒〜数分かかる。2 回目以降はキャッシュから読込まれる。詳細は [`./06-reranker.md`](./06-reranker.md) を参照。

### 4. REST API を起動

```bash
./target/release/rag-cli serve &
sleep 2
curl http://127.0.0.1:7777/health
# → {"status":"ok"}
```

### 5. API 経由で検索

```bash
curl -X POST http://127.0.0.1:7777/search \
  -H 'content-type: application/json' \
  -d '{"query":"サンプルメモの概要は？","top_n":3,"rerank":false,"generate":false}'
```

## 取込の網羅例

```bash
./target/release/rag-cli ingest data/md/note.md           # Markdown
./target/release/rag-cli ingest data/txt/changelog.txt    # プレーンテキスト
./target/release/rag-cli ingest data/svg/hello.svg        # SVG
./target/release/rag-cli ingest data/drawio/sample.drawio # drawio（要 Docling 不要）
./target/release/rag-cli ingest data/pdf/sample.pdf       # PDF（要 Docling Serve）
./target/release/rag-cli ingest https://example.com       # Web URL（要 Docling Serve）
./target/release/rag-cli ingest data/url/refs.urls        # .urls ファイル（行ごとに 1 URL）
./target/release/rag-cli ingest data/                     # ディレクトリ再帰
```

## 状態の確認とクリーンアップ

```bash
./target/release/rag-cli status                     # サービス + collection 一覧
./target/release/rag-cli reindex                    # collection 削除 + 再作成
```

詳細は [`./03-cli.md`](./03-cli.md)。

---

← [`./01-installation.md`](./01-installation.md) | → [`./03-cli.md`](./03-cli.md)
