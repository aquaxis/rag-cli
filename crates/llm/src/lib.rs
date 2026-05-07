pub mod llamacpp;
pub mod ollama;

use futures::stream::Stream;
use rag_common::{AppError, Config, Result};
use std::pin::Pin;

pub const SYSTEM_PROMPT: &str =
    "あなたは提供されたドキュメントに基づいて正確に回答するアシスタントです。\n\
以下を厳守:\n\
1. 「参考情報」のみに基づいて回答する。想像で補わない。\n\
2. 答えがない場合は「提供された情報では回答できません」と明言する。\n\
3. 末尾に [1][2] 形式で出典番号を列挙する。\n\
4. 数値・日付は原文どおりに引用する。";

#[derive(Debug, Clone)]
pub struct RetrievedDocLite {
    pub source: String,
    pub headings: Vec<String>,
    pub text: String,
}

pub fn build_user_message(question: &str, docs: &[RetrievedDocLite]) -> String {
    let mut blocks = Vec::with_capacity(docs.len());
    for (i, d) in docs.iter().enumerate() {
        let mut path = vec![d.source.clone()];
        path.extend(d.headings.clone());
        blocks.push(format!(
            "[{}] 出典: {}\n{}",
            i + 1,
            path.join(" > "),
            d.text
        ));
    }
    let ctx = blocks.join("\n\n---\n\n");
    format!("【質問】\n{question}\n\n【参考情報】\n{ctx}\n\n上記に基づいて回答してください。")
}

pub async fn generate(question: &str, docs: &[RetrievedDocLite]) -> Result<String> {
    let cfg = Config::get();
    let user = build_user_message(question, docs);
    match cfg.rag_backend.as_str() {
        "llamacpp" => llamacpp::generate(SYSTEM_PROMPT, &user).await,
        "ollama" => ollama::generate(SYSTEM_PROMPT, &user).await,
        other => Err(AppError::Config(format!("unknown RAG_BACKEND: {other}"))),
    }
}

pub type LlmStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

pub fn generate_stream(question: &str, docs: &[RetrievedDocLite]) -> LlmStream {
    let cfg = Config::get();
    let user = build_user_message(question, docs);
    match cfg.rag_backend.as_str() {
        "llamacpp" => llamacpp::generate_stream(SYSTEM_PROMPT.to_string(), user),
        "ollama" => ollama::generate_stream(SYSTEM_PROMPT.to_string(), user),
        other => Box::pin(async_stream::stream! {
            yield Err(AppError::Config(format!("unknown RAG_BACKEND: {other}")));
        }),
    }
}
