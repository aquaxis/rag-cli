//! bge-reranker-v2-m3-ONNX による cross-encoder リランカ。
//!
//! 経路:
//! 1. `RAG_RERANKER_MODEL_DIR` が設定されていればローカルディレクトリから直接読込
//! 2. 未設定時は `hf-hub` で `RERANKER_MODEL`（既定 `onnx-community/bge-reranker-v2-m3-ONNX`）の
//!    `onnx/model.onnx` `onnx/model.onnx_data` `tokenizer.json` を取得（キャッシュは
//!    `RAG_HF_CACHE_DIR` または `~/.cache/huggingface/hub/`）
//!
//! 推論は CPU + fp32。`OnceLock<RerankerSession>` でセッションを再利用する。

use ort::session::Session;
use ort::value::Tensor;
use rag_common::{AppError, Config, Result};
use std::path::PathBuf;
use std::sync::OnceLock;
use tokenizers::tokenizer::Tokenizer;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RerankItem {
    pub index: usize,
    pub score: f32,
}

struct RerankerSession {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
}

static SESSION: OnceLock<RerankerSession> = OnceLock::new();

const MAX_LEN: usize = 512;

async fn ensure_session() -> Result<&'static RerankerSession> {
    if let Some(s) = SESSION.get() {
        return Ok(s);
    }
    let cfg = Config::get();
    let (model_path, tokenizer_path) = resolve_paths(cfg).await?;

    let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| AppError::internal(format!("tokenizer load: {e}")))?;
    let _ = tokenizer.with_truncation(Some(tokenizers::tokenizer::TruncationParams {
        max_length: MAX_LEN,
        strategy: tokenizers::tokenizer::TruncationStrategy::LongestFirst,
        stride: 0,
        direction: tokenizers::tokenizer::TruncationDirection::Right,
    }));
    tokenizer.with_padding(Some(tokenizers::tokenizer::PaddingParams {
        strategy: tokenizers::tokenizer::PaddingStrategy::BatchLongest,
        direction: tokenizers::tokenizer::PaddingDirection::Right,
        pad_to_multiple_of: None,
        pad_id: 0,
        pad_type_id: 0,
        pad_token: "[PAD]".into(),
    }));

    let session = Session::builder()
        .map_err(|e| AppError::internal(format!("ort builder: {e}")))?
        .commit_from_file(&model_path)
        .map_err(|e| AppError::internal(format!("ort commit_from_file: {e}")))?;

    tracing::info!(
        model = %model_path.display(),
        tokenizer = %tokenizer_path.display(),
        "reranker session initialized"
    );

    let s = RerankerSession {
        session: Mutex::new(session),
        tokenizer,
    };
    SESSION
        .set(s)
        .map_err(|_| AppError::internal("reranker session already set"))?;
    Ok(SESSION.get().expect("just set"))
}

async fn resolve_paths(cfg: &Config) -> Result<(PathBuf, PathBuf)> {
    if let Some(dir) = &cfg.rag_reranker_model_dir {
        let d = PathBuf::from(dir);
        let model = d.join("model.onnx");
        let tokenizer = d.join("tokenizer.json");
        if !model.exists() {
            return Err(AppError::internal(format!(
                "model.onnx not found in RAG_RERANKER_MODEL_DIR={dir}"
            )));
        }
        if !tokenizer.exists() {
            return Err(AppError::internal(format!(
                "tokenizer.json not found in RAG_RERANKER_MODEL_DIR={dir}"
            )));
        }
        let data = d.join("model.onnx_data");
        if !data.exists() {
            tracing::warn!(
                "model.onnx_data not found in RAG_RERANKER_MODEL_DIR={}; \
                proceeding (only required for some models)",
                dir
            );
        }
        return Ok((model, tokenizer));
    }

    // hf-hub 1.0: HFClient::new()/builder() + client.model(owner, name)
    let client = if let Some(cache) = &cfg.rag_hf_cache_dir {
        hf_hub::HFClient::builder()
            .cache_dir(PathBuf::from(cache))
            .build()
            .map_err(|e| AppError::internal(format!("hf-hub builder: {e}")))?
    } else {
        hf_hub::HFClient::new().map_err(|e| AppError::internal(format!("hf-hub client: {e}")))?
    };

    let (owner, name) = parse_model_id(&cfg.reranker_model)?;
    let repo = client.model(owner, name);

    let model = repo
        .download_file()
        .filename("onnx/model.onnx")
        .send()
        .await
        .map_err(|e| AppError::internal(format!("hf get onnx/model.onnx: {e}")))?;
    let data = repo
        .download_file()
        .filename("onnx/model.onnx_data")
        .send()
        .await
        .map_err(|e| AppError::internal(format!("hf get onnx/model.onnx_data: {e}")))?;
    let tokenizer = repo
        .download_file()
        .filename("tokenizer.json")
        .send()
        .await
        .map_err(|e| AppError::internal(format!("hf get tokenizer.json: {e}")))?;

    let data_size = std::fs::metadata(&data).map(|m| m.len()).unwrap_or(0);
    if data_size == 0 {
        return Err(AppError::internal(
            "model.onnx_data has zero size after download",
        ));
    }
    tracing::info!(
        model = %model.display(),
        data = %data.display(),
        data_size = data_size,
        tokenizer = %tokenizer.display(),
        "reranker model files downloaded"
    );

    Ok((model, tokenizer))
}

fn parse_model_id(s: &str) -> Result<(&str, &str)> {
    let mut iter = s.splitn(2, '/');
    let owner = iter
        .next()
        .ok_or_else(|| AppError::Config(format!("invalid RERANKER_MODEL: {s}")))?;
    let name = iter
        .next()
        .ok_or_else(|| AppError::Config(format!("RERANKER_MODEL must be 'owner/name': {s}")))?;
    if owner.is_empty() || name.is_empty() {
        return Err(AppError::Config(format!("invalid RERANKER_MODEL: {s}")));
    }
    Ok((owner, name))
}

pub async fn rerank(query: &str, passages: &[String], top_n: usize) -> Result<Vec<RerankItem>> {
    if passages.is_empty() || top_n == 0 {
        return Ok(vec![]);
    }
    let session = ensure_session().await?;
    let cfg = Config::get();
    let batch = cfg.rag_rerank_batch.max(1);

    let mut all_scores: Vec<f32> = Vec::with_capacity(passages.len());
    let mut session_guard = session.session.lock().await;

    // セッションが受け付ける入力名を確認（モデルにより token_type_ids を取らないものがある）
    let model_takes_token_type_ids = session_guard
        .inputs()
        .iter()
        .any(|o| o.name() == "token_type_ids");

    for slice in passages.chunks(batch) {
        let pairs: Vec<(&str, &str)> = slice.iter().map(|p| (query, p.as_str())).collect();
        let encs = session
            .tokenizer
            .encode_batch(pairs, true)
            .map_err(|e| AppError::internal(format!("tokenize: {e}")))?;

        let bs = encs.len();
        let max_len = encs.iter().map(|e| e.len()).max().unwrap_or(0);
        if max_len == 0 {
            all_scores.extend(std::iter::repeat_n(0.0, bs));
            continue;
        }
        let mut input_ids = vec![0i64; bs * max_len];
        let mut attention_mask = vec![0i64; bs * max_len];
        let mut token_type_ids = vec![0i64; bs * max_len];
        for (i, e) in encs.iter().enumerate() {
            for (j, &id) in e.get_ids().iter().enumerate() {
                input_ids[i * max_len + j] = id as i64;
            }
            for (j, &m) in e.get_attention_mask().iter().enumerate() {
                attention_mask[i * max_len + j] = m as i64;
            }
            if model_takes_token_type_ids {
                for (j, &t) in e.get_type_ids().iter().enumerate() {
                    token_type_ids[i * max_len + j] = t as i64;
                }
            }
        }

        let shape = vec![bs as i64, max_len as i64];
        let ids_tensor = Tensor::from_array((shape.clone(), input_ids))
            .map_err(|e| AppError::internal(format!("ids tensor: {e}")))?;
        let mask_tensor = Tensor::from_array((shape.clone(), attention_mask))
            .map_err(|e| AppError::internal(format!("mask tensor: {e}")))?;

        let mut outputs = if model_takes_token_type_ids {
            let tt_tensor = Tensor::from_array((shape, token_type_ids))
                .map_err(|e| AppError::internal(format!("token_type tensor: {e}")))?;
            session_guard
                .run(ort::inputs![
                    "input_ids" => ids_tensor,
                    "attention_mask" => mask_tensor,
                    "token_type_ids" => tt_tensor,
                ])
                .map_err(|e| AppError::internal(format!("ort run: {e}")))?
        } else {
            session_guard
                .run(ort::inputs![
                    "input_ids" => ids_tensor,
                    "attention_mask" => mask_tensor,
                ])
                .map_err(|e| AppError::internal(format!("ort run: {e}")))?
        };

        // 最初の出力テンソルをスコアとして取り出す（多くの ONNX エクスポートで `logits`）
        let (_, logits_tensor) = outputs
            .iter_mut()
            .next()
            .ok_or_else(|| AppError::internal("no output tensors"))?;
        let (out_shape, data) = logits_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::internal(format!("ort extract: {e}")))?;

        // logits shape: [bs, 1] または [bs]
        let per_row = if out_shape.len() >= 2 {
            out_shape[1] as usize
        } else {
            1
        };
        for row in 0..bs {
            let s = data.get(row * per_row).copied().unwrap_or(0.0);
            all_scores.push(s);
        }
    }

    debug_assert_eq!(all_scores.len(), passages.len());

    let mut indexed: Vec<(usize, f32)> = all_scores.into_iter().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let n = top_n.min(passages.len());
    Ok(indexed
        .into_iter()
        .take(n)
        .map(|(index, score)| RerankItem { index, score })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rerank_empty_passages_returns_empty_vec() {
        let r = rerank("query", &[], 5).await.unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn rerank_top_n_zero_returns_empty_vec() {
        let r = rerank("query", &["a".into(), "b".into()], 0).await.unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn parse_model_id_ok() {
        assert_eq!(
            parse_model_id("onnx-community/bge-reranker-v2-m3-ONNX").unwrap(),
            ("onnx-community", "bge-reranker-v2-m3-ONNX")
        );
    }

    #[test]
    fn parse_model_id_rejects_missing_slash() {
        assert!(parse_model_id("noslash").is_err());
        assert!(parse_model_id("/missing-owner").is_err());
        assert!(parse_model_id("missing-name/").is_err());
    }
}
