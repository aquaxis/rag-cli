use rag_common::Config;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub source: String,
    pub chunk_id: u64,
    pub headings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<serde_json::Value>,
}

const JP_SEPARATORS: &[&str] = &[
    "\n\n", "\n", "。", "！", "？", ". ", "! ", "? ", "、", " ", "",
];

/// 入力 markdown を日本語向けに分割し、Chunk 列を返す。
///
/// 1. 見出し境界で粗くセクション化（CHUNK_SIZE*4 上限）
/// 2. 各セクションを `JP_SEPARATORS` 優先で再帰分割（CHUNK_SIZE*3 上限、overlap=CHUNK_OVERLAP*3）
/// 3. 8 文字未満を除外
/// 4. 各 chunk 先頭に見出しパス（`# h1 > # h2`）を前置
pub fn chunk_japanese(
    source: &str,
    markdown: &str,
    frontmatter: Option<serde_json::Value>,
) -> Vec<Chunk> {
    let cfg = Config::get();
    let coarse_size = cfg.chunk_size.saturating_mul(4);
    let fine_size = cfg.chunk_size.saturating_mul(3);
    let fine_overlap = cfg.chunk_overlap.saturating_mul(3);

    let sections = split_markdown_sections(markdown, coarse_size);

    let mut out: Vec<Chunk> = Vec::new();
    let mut idx: u64 = 0;
    for section in sections {
        let headings = extract_headings(&section);
        let parts = recursive_split(&section, fine_size, fine_overlap, JP_SEPARATORS);
        for body in parts {
            let text = contextualize(&headings, &body);
            if char_count(text.trim()) < 8 {
                continue;
            }
            out.push(Chunk {
                text,
                metadata: ChunkMetadata {
                    source: source.to_string(),
                    chunk_id: idx,
                    headings: headings.clone(),
                    frontmatter: frontmatter.clone(),
                },
            });
            idx += 1;
        }
    }
    out
}

fn char_count(s: &str) -> usize {
    s.graphemes(true).count()
}

fn split_markdown_sections(md: &str, max_size: usize) -> Vec<String> {
    // 見出し行（^#{1,6} ）境界で粗く分ける。ヘッダ行はそのセクション先頭。
    let mut sections: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in md.split_inclusive('\n') {
        let is_heading = is_heading_line(line);
        if is_heading && !current.is_empty() {
            sections.push(std::mem::take(&mut current));
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        sections.push(current);
    }
    if sections.is_empty() {
        return vec![md.to_string()];
    }

    // セクションごとに max_size を超える場合は素朴な再帰分割で粗く割る
    let mut out: Vec<String> = Vec::new();
    for s in sections {
        if char_count(&s) <= max_size {
            out.push(s);
        } else {
            out.extend(recursive_split(&s, max_size, 0, JP_SEPARATORS));
        }
    }
    out
}

fn is_heading_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let mut hashes = 0usize;
    for c in trimmed.chars() {
        if c == '#' {
            hashes += 1;
            if hashes > 6 {
                return false;
            }
        } else {
            return (1..=6).contains(&hashes) && (c == ' ' || c == '\t');
        }
    }
    false
}

fn recursive_split(
    text: &str,
    chunk_size: usize,
    chunk_overlap: usize,
    separators: &[&str],
) -> Vec<String> {
    if char_count(text) <= chunk_size {
        return vec![text.to_string()];
    }

    let sep = pick_separator(text, separators);
    let parts: Vec<&str> = if sep.is_empty() {
        // 文字単位フォールバック
        return split_by_chars(text, chunk_size, chunk_overlap);
    } else {
        split_keep_separator(text, sep)
    };

    // 大きすぎる断片を再帰分割
    let next_seps: Vec<&str> = separators
        .iter()
        .skip_while(|s| **s != sep)
        .skip(1)
        .copied()
        .collect();
    let mut atoms: Vec<String> = Vec::new();
    for p in parts {
        if char_count(p) <= chunk_size {
            atoms.push(p.to_string());
        } else {
            atoms.extend(recursive_split(p, chunk_size, chunk_overlap, &next_seps));
        }
    }
    merge_atoms(&atoms, chunk_size, chunk_overlap)
}

fn pick_separator<'a>(text: &str, separators: &'a [&str]) -> &'a str {
    for s in separators {
        if s.is_empty() {
            return s;
        }
        if text.contains(s) {
            return s;
        }
    }
    ""
}

fn split_keep_separator<'a>(text: &'a str, sep: &str) -> Vec<&'a str> {
    if sep.is_empty() {
        return vec![text];
    }
    // sep を末尾に残す形で分割
    let mut out: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let sep_bytes = sep.as_bytes();
    let mut i = 0usize;
    while i + sep_bytes.len() <= bytes.len() {
        if &bytes[i..i + sep_bytes.len()] == sep_bytes {
            let end = i + sep_bytes.len();
            out.push(&text[start..end]);
            start = end;
            i = end;
        } else {
            i += 1;
        }
    }
    if start < bytes.len() {
        out.push(&text[start..]);
    }
    out
}

fn split_by_chars(text: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<String> {
    let graphs: Vec<&str> = text.graphemes(true).collect();
    if graphs.len() <= chunk_size {
        return vec![text.to_string()];
    }
    let step = chunk_size.saturating_sub(chunk_overlap).max(1);
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < graphs.len() {
        let end = (i + chunk_size).min(graphs.len());
        out.push(graphs[i..end].concat());
        if end == graphs.len() {
            break;
        }
        i += step;
    }
    out
}

fn merge_atoms(atoms: &[String], chunk_size: usize, chunk_overlap: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut buf_len = 0usize;
    for a in atoms {
        let alen = char_count(a);
        if buf_len + alen <= chunk_size {
            buf.push_str(a);
            buf_len += alen;
        } else {
            if !buf.is_empty() {
                out.push(buf.clone());
                // overlap 用に末尾を保持
                if chunk_overlap > 0 {
                    let graphs: Vec<&str> = buf.graphemes(true).collect();
                    let take = chunk_overlap.min(graphs.len());
                    let tail = graphs[graphs.len() - take..].concat();
                    buf = tail;
                    buf_len = char_count(&buf);
                } else {
                    buf.clear();
                    buf_len = 0;
                }
            }
            buf.push_str(a);
            buf_len += alen;
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

fn extract_headings(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in md.split('\n') {
        if let Some(t) = parse_heading(line) {
            out.push(t);
        }
    }
    out
}

fn parse_heading(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let mut hashes = 0usize;
    for (i, c) in trimmed.char_indices() {
        if c == '#' {
            hashes += 1;
            if hashes > 6 {
                return None;
            }
        } else {
            if !(1..=6).contains(&hashes) {
                return None;
            }
            if c == ' ' || c == '\t' {
                let rest = trimmed[i..].trim();
                if rest.is_empty() {
                    return None;
                }
                return Some(rest.trim_end().to_string());
            }
            return None;
        }
    }
    None
}

fn contextualize(headings: &[String], body: &str) -> String {
    if headings.is_empty() {
        return body.to_string();
    }
    let prefix = headings
        .iter()
        .map(|h| format!("# {h}"))
        .collect::<Vec<_>>()
        .join(" > ");
    format!("{prefix}\n\n{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headings() {
        assert_eq!(parse_heading("# 概要"), Some("概要".to_string()));
        assert_eq!(parse_heading("### foo bar"), Some("foo bar".to_string()));
        assert_eq!(parse_heading("####### too many"), None);
        assert_eq!(parse_heading("not heading"), None);
    }

    #[test]
    fn small_text_yields_single_chunk() {
        let src = "src.md";
        let md = "# 概要\n\n本書はサンプルメモ。";
        let chunks = chunk_japanese(src, md, None);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("# 概要"));
    }

    #[test]
    fn excludes_too_short_chunks() {
        let src = "src.md";
        let md = "a";
        let chunks = chunk_japanese(src, md, None);
        assert_eq!(chunks.len(), 0);
    }
}
