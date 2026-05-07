use super::{ConvertedDoc, DocMetadata};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rag_common::{AppError, Result};
use std::path::Path;

#[derive(Debug, Clone)]
struct SvgTextEl {
    kind: String,
    x: Option<String>,
    y: Option<String>,
    text: String,
}

pub async fn convert_svg(path: &Path) -> Result<ConvertedDoc> {
    let bytes = tokio::fs::read(path).await?;
    let xml = String::from_utf8(bytes).map_err(AppError::parse)?;

    let elements = extract_text_elements(&xml)?;
    let embedded = extract_embedded_drawio_content(&xml)?;

    let basename = path.file_name().and_then(|s| s.to_str()).unwrap_or("svg");
    let mut lines = Vec::<String>::new();
    lines.push(format!("# SVG: {basename}"));
    lines.push(String::new());
    for el in &elements {
        let pos = match (&el.x, &el.y) {
            (Some(x), Some(y)) => format!(" (x={x}, y={y})"),
            _ => String::new(),
        };
        lines.push(format!("- [{}]{pos} {}", el.kind, el.text));
    }

    if let Some(content) = embedded {
        let drawio_md = super::drawio::extract_mx_cells(&content);
        if !drawio_md.is_empty() {
            lines.push(String::new());
            lines.push("## Embedded drawio cells".into());
            lines.push(String::new());
            lines.push(drawio_md);
        }
    }

    Ok(ConvertedDoc {
        source: path.display().to_string(),
        markdown: lines.join("\n"),
        metadata: DocMetadata::File {
            ext: "svg".into(),
            frontmatter: None,
            title: Some(basename.to_string()),
        },
    })
}

fn extract_text_elements(xml: &str) -> Result<Vec<SvgTextEl>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut out = Vec::<SvgTextEl>::new();
    let mut stack: Vec<(String, Option<String>, Option<String>, String)> = Vec::new();

    let target = |tag: &str| matches!(tag, "text" | "tspan" | "title" | "desc");

    loop {
        match reader.read_event() {
            Err(e) => return Err(AppError::parse(format!("svg parse: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut x: Option<String> = None;
                let mut y: Option<String> = None;
                if target(&tag) {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = attr
                            .unescape_value()
                            .map(|c| c.to_string())
                            .unwrap_or_default();
                        if key == "x" {
                            x = Some(val);
                        } else if key == "y" {
                            y = Some(val);
                        }
                    }
                }
                stack.push((tag, x, y, String::new()));
            }
            Ok(Event::End(_)) => {
                if let Some((tag, x, y, buf)) = stack.pop() {
                    if target(&tag) && !buf.trim().is_empty() {
                        out.push(SvgTextEl {
                            kind: tag,
                            x,
                            y,
                            text: buf.trim().to_string(),
                        });
                    }
                    if let Some(parent) = stack.last_mut() {
                        parent.3.push_str(&buf);
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(top) = stack.last_mut() {
                    let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                    top.3.push_str(&s);
                }
            }
            Ok(Event::CData(t)) => {
                if let Some(top) = stack.last_mut() {
                    top.3.push_str(&String::from_utf8_lossy(&t));
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if target(&tag) {
                    // 空要素にテキストはないが、stack 操作不要
                    let _ = tag;
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// `<svg content="...">` 属性に埋め込まれた drawio mxGraph XML を返す
fn extract_embedded_drawio_content(xml: &str) -> Result<Option<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Err(_) => return Ok(None),
            Ok(Event::Eof) => return Ok(None),
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "svg" {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        if key == "content" {
                            let val = attr
                                .unescape_value()
                                .map(|c| c.to_string())
                                .unwrap_or_default();
                            if val.trim_start().starts_with('<') {
                                return Ok(Some(val));
                            }
                        }
                    }
                    return Ok(None);
                }
            }
            _ => {}
        }
    }
}
