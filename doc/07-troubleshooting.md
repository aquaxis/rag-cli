# 07. トラブルシュート

よくある問題と対処の早見表。

## サービス疎通

### `qdrant: down`

```bash
rag-cli status
# → "qdrant": "down"
```

原因と対処:

| 原因 | 対処 |
|------|------|
| Qdrant 未起動 | `docker compose up -d qdrant` で起動 |
| URL 不一致 | `QDRANT_URL` を確認（既定 `http://127.0.0.1:6333`） |
| TLS / 認証必須 | `QDRANT_API_KEY` を `.env` に設定 |

```bash
curl -fsS http://127.0.0.1:6333/readyz
# → "all shards are ready"
```

### `ollama: down`

```bash
rag-cli status
# → "ollama": "down"
```

| 原因 | 対処 |
|------|------|
| Ollama 未起動 | `ollama serve` または `docker compose up -d ollama` |
| モデル未投入 | `docker exec rag-ollama ollama pull bge-m3` |
| ホスト不一致 | `OLLAMA_HOST` を確認（既定 `http://127.0.0.1:11434`） |

```bash
curl -fsS http://127.0.0.1:11434/api/tags | jq '.models[].name'
```

### `docling: down`

PDF / 画像 / Web URL 取込のみで必要。SVG / drawio / Markdown / テキストは Docling 不要。

| 原因 | 対処 |
|------|------|
| Docling Serve 未起動 | `docker compose up -d docling` |
| OOM | `docker compose logs docling` で OOM を確認、メモリ増設 / バッチ縮小 |

## 取込のエラー

### `Dim mismatch: expected 1024, got N`

| 原因 | 対処 |
|------|------|
| 別の embed モデルを `OLLAMA_EMBED_MODEL` に設定 | `EMBED_DIM` をモデルに合わせる、または bge-m3 に戻す |
| `LLAMACPP_EMBED_MODEL` が GGUF 化された別モデル | dim を確認して `EMBED_DIM` を更新 |

### `Docling errors: ...`

Docling Serve からのエラー。考えられる原因:

| 原因 | 対処 |
|------|------|
| ファイル破損 | 別の PDF / 画像で再試行 |
| OOM（大量ページ PDF） | ページを分割、または並列度を下げる |
| OCR エンジン未準備 | Docling コンテナが初回 DL 中の可能性あり、起動ログを確認 |

### `Empty markdown from URL: <url>`

URL の取得は成功したが Markdown が空。原因:

- robots.txt や WAF でブロックされている
- JavaScript レンダリングが必要なページ（Docling は対応していない）
- 認証が必要

Docling のスコープ外。事前にダウンロードしてローカルファイル取込に切替える。

### 単一ファイルが `unsupported extension: <ext>` で失敗

対応拡張子は `.pdf` `.png` `.jpg` `.jpeg` `.tiff` `.bmp` `.svg` `.drawio` `.drawio.svg` `.md` `.markdown` `.txt` `.log` `.rst` `.urls`。それ以外は事前に変換する。

## リランカ関連

### 初回起動が遅い、応答がない

bge-reranker-v2-m3-ONNX の初回 DL は約 600 MB。ネットワーク帯域に応じて数十秒〜数分かかる。

```bash
LOG_LEVEL=info rag-cli search "..." --top-n 3 2>&1 | grep "reranker model"
# → "reranker model files downloaded" が表示されれば DL 完了
```

### `model.onnx_data has zero size after download`

DL が中断されて 0 byte ファイルが残った可能性。キャッシュを削除して再試行:

```bash
rm -rf ~/.cache/huggingface/hub/models--onnx-community--bge-reranker-v2-m3-ONNX
rag-cli search "..." --top-n 3
```

### HF Hub に接続できない（オフライン環境）

事前 DL 後、`RAG_RERANKER_MODEL_DIR` でローカルディレクトリを指定する。詳細は [`./06-reranker.md`](./06-reranker.md)。

### メモリ不足で OOM kill される

```bash
RAG_RERANK_BATCH=1 rag-cli search "..." --top-n 3
```

それでも不足する場合は `--no-rerank` で運用する。

## API サーバ

### `Address already in use`（ポート 7777 衝突）

```bash
rag-cli serve --port 7780
# または
RAG_API_PORT=7780 rag-cli serve
```

`lsof -i :7777` で何が使っているか確認できる。

### CORS でブラウザから叩けない

API はすべてのオリジンを許可（`CorsLayer::permissive()`）するため通常は問題ないが、リバースプロキシ経由の場合はプロキシ側の設定を確認。

## ビルド / Cargo

### `error: failed to download` / `Network unreachable`

ネットワーク制約で crates.io / HF Hub に接続できない場合:

```bash
# crates.io: vendored 依存を使う
cargo vendor
# .cargo/config.toml に `replace-with = "vendored-sources"` を追加
```

### `linker error` / `cannot find -lonnxruntime`

`ort` の `download-binaries` features が無効化されると発生する。`Cargo.toml` の workspace.dependencies で `ort` が default features を有効にしていることを確認。

```toml
ort = { version = "2.0.0-rc.12" }  # default features を維持
```

## ログレベル

詳細なログを出すには:

```bash
LOG_LEVEL=debug rag-cli search "..."
# または モジュール別
RUST_LOG=rag_search=debug,rag_pipeline=info rag-cli search "..."
```

エラーの詳細を確認するには `LOG_LEVEL=trace`。

---

← [`./06-reranker.md`](./06-reranker.md) | → [`./08-architecture.md`](./08-architecture.md)
