use super::{ConvertedDoc, DocMetadata};
use base64::Engine;
use flate2::read::DeflateDecoder;
use percent_encoding::percent_decode_str;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use rag_common::{AppError, Result};
use std::io::Read;
use std::path::Path;

pub async fn convert_drawio(path: &Path) -> Result<ConvertedDoc> {
    let bytes = tokio::fs::read(path).await?;
    let xml = String::from_utf8(bytes).map_err(AppError::parse)?;

    let diagrams = collect_diagrams(&xml)?;

    let basename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("drawio");
    let mut lines = Vec::<String>::new();
    lines.push(format!("# drawio: {basename}"));
    lines.push(String::new());
    for (i, (name, content)) in diagrams.iter().enumerate() {
        let inner = decompress_diagram(content);
        let suffix = match name {
            Some(n) => format!(": {n}"),
            None => String::new(),
        };
        lines.push(format!("## Diagram {}{}", i + 1, suffix));
        lines.push(String::new());
        lines.push(extract_mx_cells(&inner));
        lines.push(String::new());
    }

    Ok(ConvertedDoc {
        source: path.display().to_string(),
        markdown: lines.join("\n"),
        metadata: DocMetadata::File {
            ext: "drawio".into(),
            frontmatter: None,
            title: Some(basename.to_string()),
        },
    })
}

/// `<mxfile><diagram name="...">{content}</diagram>` を全て取得する
fn collect_diagrams(xml: &str) -> Result<Vec<(Option<String>, String)>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out: Vec<(Option<String>, String)> = Vec::new();

    let mut current_name: Option<String> = None;
    let mut depth: usize = 0;
    let mut buf = String::new();
    let mut inside = false;

    loop {
        match reader.read_event() {
            Err(e) => return Err(AppError::parse(format!("drawio parse: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "diagram" {
                    inside = true;
                    depth = 1;
                    current_name = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"name")
                        .and_then(|a| a.unescape_value().ok().map(|c| c.to_string()));
                    buf.clear();
                } else if inside {
                    depth += 1;
                    // 子要素含めて raw 形を残せないため、ここではシリアライズし直す
                    buf.push('<');
                    buf.push_str(&tag);
                    for attr in e.attributes().flatten() {
                        let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let v = attr
                            .unescape_value()
                            .map(|c| c.to_string())
                            .unwrap_or_default();
                        buf.push(' ');
                        buf.push_str(&k);
                        buf.push_str("=\"");
                        buf.push_str(&v.replace('"', "&quot;"));
                        buf.push('"');
                    }
                    buf.push('>');
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "diagram" && inside {
                    out.push((current_name.take(), buf.trim().to_string()));
                    inside = false;
                    depth = 0;
                    buf.clear();
                } else if inside {
                    depth = depth.saturating_sub(1);
                    buf.push_str("</");
                    buf.push_str(&tag);
                    buf.push('>');
                }
            }
            Ok(Event::Empty(e)) if inside => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                buf.push('<');
                buf.push_str(&tag);
                for attr in e.attributes().flatten() {
                    let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                    let v = attr
                        .unescape_value()
                        .map(|c| c.to_string())
                        .unwrap_or_default();
                    buf.push(' ');
                    buf.push_str(&k);
                    buf.push_str("=\"");
                    buf.push_str(&v.replace('"', "&quot;"));
                    buf.push('"');
                }
                buf.push_str("/>");
            }
            Ok(Event::Text(t)) if inside => {
                let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                buf.push_str(&s);
            }
            _ => {}
        }
    }
    Ok(out)
}

pub fn decompress_diagram(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.starts_with("<mxGraphModel") || trimmed.starts_with("<?xml") {
        return trimmed.to_string();
    }
    // base64 → bytes → DeflateRaw → percent-decode
    let bytes = match base64::engine::general_purpose::STANDARD.decode(trimmed) {
        Ok(b) => b,
        Err(_) => return trimmed.to_string(),
    };
    let mut decoder = DeflateDecoder::new(&bytes[..]);
    let mut inflated = String::new();
    if decoder.read_to_string(&mut inflated).is_err() {
        return trimmed.to_string();
    }
    percent_decode_str(&inflated)
        .decode_utf8_lossy()
        .to_string()
}

pub fn extract_mx_cells(mx_xml: &str) -> String {
    let mut reader = Reader::from_str(mx_xml);
    reader.config_mut().trim_text(false);
    let mut out: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Err(_) | Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "mxCell" || tag == "UserObject" {
                    let mut id: Option<String> = None;
                    let mut label: Option<String> = None;
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = attr
                            .unescape_value()
                            .map(|c| c.to_string())
                            .unwrap_or_default();
                        if key == "id" {
                            id = Some(val);
                        } else if key == "value" || key == "label" {
                            label = Some(val);
                        }
                    }
                    if let Some(l) = label.filter(|s| !s.trim().is_empty()) {
                        let stripped = strip_html(&l);
                        let line = match id {
                            Some(i) => format!("- {} (id={})", stripped.trim(), i),
                            None => format!("- {}", stripped.trim()),
                        };
                        out.push(line);
                    }
                }
            }
            _ => {}
        }
    }
    out.join("\n")
}

fn strip_html(s: &str) -> String {
    let re_tag = regex::Regex::new(r"<[^>]+>").unwrap();
    let re_ws = regex::Regex::new(r"\s+").unwrap();
    let s = re_tag.replace_all(s, " ");
    let s = s.replace("&nbsp;", " ");
    re_ws.replace_all(&s, " ").to_string()
}
