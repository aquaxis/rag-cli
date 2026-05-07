use clap::{Parser, Subcommand};
use rag_common::{logger, AppError, Config, Result};
use rag_llm::{generate, RetrievedDocLite};
use rag_pipeline::{expand_path, ingest_path, ingest_paths, retrieve, RetrieveOpts};
use rag_search::{ensure_collection, get_qdrant_client};

#[derive(Parser, Debug)]
#[command(name = "rag-cli", version, about = "Local Standalone RAG (Rust)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// ファイル / ディレクトリ / URL / .urls を取込
    Ingest { target: String },
    /// 検索 + LLM 応答
    Search {
        query: String,
        #[arg(short = 'k', long, default_value_t = 20)]
        top_k: u64,
        #[arg(short = 'n', long, default_value_t = 5)]
        top_n: u64,
        /// リランク無効化
        #[arg(long)]
        no_rerank: bool,
        /// LLM 応答生成を無効化
        #[arg(long)]
        no_generate: bool,
    },
    /// 各サービスのヘルスと collection
    Status,
    /// collection を削除して再作成
    Reindex,
    /// Hono 互換 HTTP API を起動
    Serve {
        #[arg(short, long)]
        port: Option<u16>,
    },
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    // CLI 引数で環境変数を上書きしてから Config をロードする
    if let Cmd::Serve { port: Some(p) } = &cli.cmd {
        std::env::set_var("RAG_API_PORT", p.to_string());
    }
    let _ = Config::load();
    logger::init();

    match cli.cmd {
        Cmd::Ingest { target } => cmd_ingest(&target).await,
        Cmd::Search {
            query,
            top_k,
            top_n,
            no_rerank,
            no_generate,
        } => cmd_search(&query, top_k, top_n, !no_rerank, !no_generate).await,
        Cmd::Status => cmd_status().await,
        Cmd::Reindex => cmd_reindex().await,
        Cmd::Serve { .. } => rag_api::run().await,
    }
}

async fn cmd_ingest(target: &str) -> Result<()> {
    let expanded = expand_path(target).await?;
    if expanded.len() == 1 {
        let r = ingest_path(&expanded[0]).await?;
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "source": expanded[0],
                "chunks": r.chunks,
            }))
            .unwrap()
        );
    } else {
        let total = expanded.len();
        let stats = ingest_paths(expanded).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ingested": stats.ingested,
                "chunks": stats.chunks,
                "errors": stats.errors,
                "total": total,
            }))
            .unwrap()
        );
    }
    Ok(())
}

async fn cmd_search(query: &str, top_k: u64, top_n: u64, rerank: bool, do_gen: bool) -> Result<()> {
    let docs = retrieve(
        query,
        RetrieveOpts {
            top_k: Some(top_k),
            top_n: Some(top_n),
            rerank: Some(rerank),
            filter: None,
        },
    )
    .await?;

    if do_gen {
        let lite: Vec<RetrievedDocLite> = docs
            .iter()
            .map(|d| RetrievedDocLite {
                source: d.source.clone(),
                headings: d.headings.clone(),
                text: d.text.clone(),
            })
            .collect();
        let answer = generate(query, &lite).await?;
        println!("=== 回答 ===");
        println!("{answer}");
    }
    println!("\n=== 出典 ===");
    for (i, d) in docs.iter().enumerate() {
        let head = if d.headings.is_empty() {
            String::new()
        } else {
            format!(" > {}", d.headings.join(" > "))
        };
        let rs = match d.rerank_score {
            Some(s) => format!("{s:.3}"),
            None => "n/a".to_string(),
        };
        println!("[{}] {}{} (rerank={})", i + 1, d.source, head, rs);
    }
    Ok(())
}

async fn cmd_status() -> Result<()> {
    let cfg = Config::get();
    let http = reqwest::Client::new();
    let q = http
        .get(format!("{}/readyz", cfg.qdrant_url.trim_end_matches('/')))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    let o = http
        .get(format!(
            "{}/api/tags",
            cfg.ollama_host.trim_end_matches('/')
        ))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    let d = http
        .get(format!("{}/health", cfg.docling_url.trim_end_matches('/')))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let client = get_qdrant_client();
    let collections = client
        .raw_get("/collections")
        .await
        .ok()
        .and_then(|v| v.get("result").cloned())
        .and_then(|v| v.get("collections").cloned())
        .unwrap_or(serde_json::Value::Array(vec![]));

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "qdrant": if q { "ok" } else { "down" },
            "ollama": if o { "ok" } else { "down" },
            "docling": if d { "ok" } else { "down" },
            "backend": cfg.rag_backend,
            "collections": collections,
        }))
        .unwrap()
    );
    Ok(())
}

async fn cmd_reindex() -> Result<()> {
    let cfg = Config::get();
    let client = get_qdrant_client();
    ensure_collection(&client, true).await?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "collection": cfg.qdrant_collection,
            "recreated": true,
        }))
        .unwrap()
    );
    Ok(())
}

#[allow(dead_code)]
fn _unused(_e: AppError) {}
