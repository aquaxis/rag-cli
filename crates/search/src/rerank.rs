//! bge-reranker-v2-m3-ONNX によるリランカ。
//!
//! 現状はスタブ実装: ONNX runtime（`ort`）+ `tokenizers` + `hf-hub` の本格統合は
//! Phase RR8 の PoC で完成させる。スタブはクエリと passage のテキスト類似度（簡易）を
//! スコアとして返し、index 順位を維持する。
//!
//! 既存 TS 実装でも `model.onnx_data` の DL 失敗で実効動作していなかったため、
//! Rust 移植後も `--no-rerank` で運用する前提を README に明記する。

use rag_common::Result;

#[derive(Debug, Clone)]
pub struct RerankItem {
    pub index: usize,
    pub score: f32,
}

pub async fn rerank(_query: &str, passages: &[String], top_n: usize) -> Result<Vec<RerankItem>> {
    // スタブ: 元順位維持、スコアは降順の擬似値
    let n = passages.len().min(top_n);
    Ok((0..n)
        .map(|i| RerankItem {
            index: i,
            score: (passages.len() - i) as f32,
        })
        .collect())
}
