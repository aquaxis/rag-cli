use super::{ConvertedDoc, DocMetadata};
use rag_common::{AppError, Config, Result};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct DoclingSourceResponse {
    #[serde(default)]
    document: Option<DoclingDoc>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    errors: Option<Vec<DoclingError>>,
}

#[derive(Debug, Deserialize)]
struct DoclingDoc {
    #[serde(default)]
    md_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DoclingError {
    #[serde(default)]
    error_message: String,
}

pub async fn convert_web(url: &str) -> Result<ConvertedDoc> {
    if !crate::is_url(url) {
        return Err(AppError::Validation(format!("Not an HTTP(S) URL: {url}")));
    }
    let cfg = Config::get();
    let body = json!({
        "sources": [{ "kind": "http", "url": url }],
        "options": {
            "from_formats": ["pdf", "image", "docx", "pptx", "html", "md"],
            "to_formats": ["md"],
            "do_ocr": true,
            "ocr_engine": "easyocr",
            "ocr_lang": ["ja", "en"],
            "table_mode": "accurate",
            "image_export_mode": "placeholder",
            "abort_on_error": false,
            "return_as_file": false,
        },
    });

    let endpoint = format!(
        "{}/v1alpha/convert/source",
        cfg.docling_url.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(AppError::http)?;

    let resp = client.post(&endpoint).json(&body).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Docling(format!("status={status}, body={text}")));
    }
    let parsed: DoclingSourceResponse = resp.json().await?;
    if parsed.status.as_deref() != Some("success") {
        if let Some(errs) = parsed.errors {
            let msg = errs
                .iter()
                .map(|e| e.error_message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(AppError::Docling(msg));
        }
    }
    let md = parsed
        .document
        .and_then(|d| d.md_content)
        .unwrap_or_default();
    if md.is_empty() {
        return Err(AppError::Docling(format!("Empty markdown from URL: {url}")));
    }

    Ok(ConvertedDoc {
        source: url.to_string(),
        markdown: md,
        metadata: DocMetadata::Web {
            url: url.to_string(),
        },
    })
}

pub async fn read_url_list(path: &Path) -> Result<Vec<String>> {
    let bytes = tokio::fs::read(path).await?;
    let raw = String::from_utf8(bytes).map_err(AppError::parse)?;
    let mut out = Vec::new();
    for line in raw.split('\n') {
        let t = line.trim().trim_end_matches('\r').trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        out.push(t.to_string());
    }
    Ok(out)
}
