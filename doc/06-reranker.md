# 06. リランカ（bge-reranker-v2-m3）

`rag-cli search` のリランクは `bge-reranker-v2-m3-ONNX` を CPU + fp32 で推論する。`ort` + `tokenizers` + `hf-hub` クレートを使用し、既存 collection に対して順位を再評価する。

## なぜリランカが必要か

bi-encoder（`bge-m3`）の埋込類似度は速いが、文脈の細部までは反映しない。クロスエンコーダの bge-reranker-v2-m3 はクエリと passage を同時に見るためスコア精度が高く、上位 5〜10 件の並べ替えに有効。

例: クエリ「web 取込の対応日」に対する順位（ローカル検証）

| 順位 | リランクなし | リランクあり |
|------|-------------|-------------|
| 1 | `changelog.txt` | `changelog.txt`（rerank=0.137） |
| 2 | `hello.svg` | `note.md`（rerank=-11.025） |
| 3 | `note.md`（upload） | `note.md`（rerank=-11.025） |
| 4 | `note.md`（md） | `hello.svg`（rerank=-11.035） |

無関係な SVG が 2 位 → 4 位に降格、関連性の高いノートが上位に繰り上がる。

## モデルファイルの取得

リランカは初回起動時に以下の 3 ファイルを取得する:

| ファイル | サイズ目安 |
|---------|-----------|
| `onnx/model.onnx` | ~50 MB |
| `onnx/model.onnx_data` | ~570 MB（外部データ） |
| `tokenizer.json` | ~10 MB |

合計 約 600 MB。HF Hub からの DL 経路は以下の優先順位で決定する:

1. `RAG_RERANKER_MODEL_DIR=/path` が設定されていれば、そのディレクトリから直接読込（DL なし）
2. それ以外は `hf-hub` クレートで `RERANKER_MODEL`（既定 `onnx-community/bge-reranker-v2-m3-ONNX`）を取得
   - キャッシュディレクトリ: `RAG_HF_CACHE_DIR` または既定 `~/.cache/huggingface/hub/`

## オフライン環境での運用

事前にモデルを DL し、`RAG_RERANKER_MODEL_DIR` で指定する:

```bash
# 1. ネット接続環境でモデルを DL（HF CLI または `rag-cli search` を一度実行）
rag-cli search "test" --top-n 1
# → ~/.cache/huggingface/hub/models--onnx-community--bge-reranker-v2-m3-ONNX/snapshots/<hash>/

# 2. オフライン環境にコピー
rsync -av ~/.cache/huggingface/hub/models--onnx-community--bge-reranker-v2-m3-ONNX/snapshots/<hash>/ \
  /opt/rag/reranker/

# 3. RAG_RERANKER_MODEL_DIR を設定
export RAG_RERANKER_MODEL_DIR=/opt/rag/reranker
rag-cli search "..." --top-n 3
```

`RAG_RERANKER_MODEL_DIR` 配下に必要なレイアウト:

```text
/opt/rag/reranker/
├── model.onnx
├── model.onnx_data
└── tokenizer.json
```

`onnx/` サブディレクトリは不要。スナップショットディレクトリから 3 ファイルを直接持ってくる（または symlink でも可）。

## 性能

| シナリオ | レイテンシ |
|---------|------------|
| Cold start（CLI 1 回起動、初回のみ） | ~2.3 s（モデル読込含む） |
| Warm（API サーバ、2 回目以降） | ~0.4 s（OnceLock + Mutex で session 再利用） |

CLI は 1 回ごとにプロセスが終了するため、毎回 cold start となる。バッチ処理や API 利用時は `rag-cli serve` を起動して常駐させるのが効率的。

## バッチサイズと メモリ

| 環境変数 | 既定 | 説明 |
|---------|------|------|
| `RAG_RERANK_BATCH` | `8` | 1 推論で処理する passage 数 |

メモリ目安:
- セッション常駐: ~1.5 GB（fp32）
- 推論時の一時メモリ: バッチ × 系列長 × 2 (i64) × ~3 倍

メモリ不足時は `RAG_RERANK_BATCH=1` に下げる。

## リランクをスキップする

```bash
rag-cli search "..." --top-n 5 --no-rerank
```

または API 経由:

```json
{"query":"...", "top_n":5, "rerank":false}
```

リランクなしでも bi-encoder のスコアで上位を返すため、要約や粗い検索には十分な場合が多い。

## 内部実装の概要

- セッションは `OnceLock<RerankerSession>` でグローバルキャッシュ
- `Session::run` が `&mut self` のため、Session を `tokio::sync::Mutex` でラップ
- `Tensor::from_array((shape, Vec<i64>))` のタプル形式を採用（ndarray のバージョン衝突を回避）
- 入力は `input_ids` + `attention_mask`（XLM-RoBERTa 系のため `token_type_ids` は不要）
  - 古い BERT 系モデルを `RERANKER_MODEL` に指定した場合、`Session::inputs()` を見て `token_type_ids` の有無を動的に判定
- `tokenizer.with_truncation(max_length=512)` + `with_padding(BatchLongest)`
- 出力 `logits[bs, 1]` を float32 で抽出し、降順ソート → `top_n` を返却

ソース: [`crates/search/src/rerank.rs`](../crates/search/src/rerank.rs)

---

← [`./05-configuration.md`](./05-configuration.md) | → [`./07-troubleshooting.md`](./07-troubleshooting.md)
