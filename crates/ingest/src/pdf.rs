use super::{ConvertedDoc, DocMetadata};
use rag_common::{AppError, Config, Result};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct DoclingResponse {
    #[serde(default)]
    document: Option<DoclingDocument>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    errors: Option<Vec<DoclingError>>,
}

#[derive(Debug, Deserialize)]
struct DoclingDocument {
    #[serde(default)]
    md_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DoclingError {
    #[serde(default)]
    error_message: String,
}

pub async fn convert_pdf_or_image(path: &Path) -> Result<ConvertedDoc> {
    let cfg = Config::get();
    let bytes = tokio::fs::read(path).await?;
    let basename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    let parameters = json!({
        "from_formats": ["pdf", "image", "docx", "pptx", "html", "md"],
        "to_formats": ["md"],
        "do_ocr": true,
        "ocr_engine": "easyocr",
        "ocr_lang": ["ja", "en"],
        "table_mode": "accurate",
        "image_export_mode": "placeholder",
        "abort_on_error": false,
        "return_as_file": false,
    });

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(basename.to_string())
        .mime_str(&mime)
        .map_err(AppError::http)?;
    let form = reqwest::multipart::Form::new()
        .part("files", part)
        .text("parameters", parameters.to_string());

    let url = format!(
        "{}/v1alpha/convert/file",
        cfg.docling_url.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(AppError::http)?;

    let resp = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(AppError::http)?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Docling(format!("status={status}, body={body}")));
    }

    let parsed: DoclingResponse = resp.json().await.map_err(AppError::http)?;
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
        return Err(AppError::Docling("empty markdown from Docling".into()));
    }

    Ok(ConvertedDoc {
        source: path.display().to_string(),
        markdown: md,
        metadata: DocMetadata::File {
            ext: path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("pdf")
                .to_string(),
            frontmatter: None,
            title: Some(basename.to_string()),
        },
    })
}
