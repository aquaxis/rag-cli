pub mod drawio;
pub mod markdown;
pub mod pdf;
pub mod svg;
pub mod web;

use rag_common::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "kind")]
pub enum DocMetadata {
    File {
        ext: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        frontmatter: Option<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Web {
        url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertedDoc {
    pub source: String,
    pub markdown: String,
    pub metadata: DocMetadata,
}

impl ConvertedDoc {
    pub fn kind_str(&self) -> &'static str {
        match &self.metadata {
            DocMetadata::File { .. } => "file",
            DocMetadata::Web { .. } => "web",
        }
    }

    pub fn url(&self) -> Option<&str> {
        match &self.metadata {
            DocMetadata::Web { url } => Some(url),
            _ => None,
        }
    }
}

pub fn is_url(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub async fn convert_any(input: &str) -> Result<ConvertedDoc> {
    if is_url(input) {
        return web::convert_web(input).await;
    }
    let path = Path::new(input);
    if !path.exists() {
        return Err(AppError::NotFound(input.to_string()));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // .drawio.svg は SVG として扱う（拡張子は svg）
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".drawio.svg") {
        return svg::convert_svg(path).await;
    }

    match ext.as_str() {
        "pdf" | "png" | "jpg" | "jpeg" | "tiff" | "bmp" => pdf::convert_pdf_or_image(path).await,
        "svg" => svg::convert_svg(path).await,
        "drawio" => drawio::convert_drawio(path).await,
        "md" | "markdown" => markdown::convert_markdown(path).await,
        "txt" | "log" | "rst" => markdown::convert_text(path).await,
        other => Err(AppError::Validation(format!(
            "unsupported extension: {other}"
        ))),
    }
}
