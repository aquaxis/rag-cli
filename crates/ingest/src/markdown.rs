use super::{ConvertedDoc, DocMetadata};
use rag_common::{AppError, Result};
use std::path::Path;

/// `---\nYAML\n---\n本文` 形式の frontmatter を分離する（gray_matter 風）。
/// Returns `(content, frontmatter_value)`。
fn split_frontmatter(raw: &str) -> (String, Option<serde_json::Value>) {
    let trimmed = raw.trim_start_matches('\u{FEFF}');
    if !trimmed.starts_with("---") {
        return (trimmed.to_string(), None);
    }
    let after_first = match trimmed.strip_prefix("---") {
        Some(s) => s,
        None => return (trimmed.to_string(), None),
    };
    let after_first = after_first
        .trim_start_matches('\r')
        .trim_start_matches('\n');
    let close = match after_first.find("\n---") {
        Some(i) => i,
        None => return (trimmed.to_string(), None),
    };
    let yaml = &after_first[..close];
    let mut rest = &after_first[close + 4..];
    rest = rest.trim_start_matches('\r').trim_start_matches('\n');

    let value = match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
        Ok(yv) => serde_json::to_value(yv).ok(),
        Err(_) => None,
    };
    (rest.to_string(), value)
}

pub async fn convert_markdown(path: &Path) -> Result<ConvertedDoc> {
    let bytes = tokio::fs::read(path).await?;
    let raw = String::from_utf8(bytes).map_err(AppError::parse)?;

    let (content, frontmatter) = split_frontmatter(&raw);
    let title = frontmatter
        .as_ref()
        .and_then(|v| v.get("title"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md")
        .to_string();

    Ok(ConvertedDoc {
        source: path.display().to_string(),
        markdown: content,
        metadata: DocMetadata::File {
            ext,
            frontmatter,
            title,
        },
    })
}

pub async fn convert_text(path: &Path) -> Result<ConvertedDoc> {
    let bytes = tokio::fs::read(path).await?;
    let mut content = String::from_utf8(bytes).map_err(AppError::parse)?;

    // BOM 除去
    if content.starts_with('\u{FEFF}') {
        content = content.trim_start_matches('\u{FEFF}').to_string();
    }
    // CRLF -> LF
    let content = content.replace("\r\n", "\n");
    // 3 連続以上の空行を 2 行に圧縮
    let re = regex::Regex::new(r"\n{3,}").map_err(AppError::parse)?;
    let content = re.replace_all(&content, "\n\n").to_string();

    let basename = path.file_name().and_then(|n| n.to_str()).unwrap_or("text");
    let markdown = format!("# {basename}\n\n{content}");

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt")
        .to_string();

    Ok(ConvertedDoc {
        source: path.display().to_string(),
        markdown,
        metadata: DocMetadata::File {
            ext,
            frontmatter: None,
            title: Some(basename.to_string()),
        },
    })
}
