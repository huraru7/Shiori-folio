//! Markdownの見出し単位でのチャンク分け(`services/rag/chunking.py` と同じ)。
//!
//! 見出し単位を優先し、1チャンクは目安500トークン。見出し1つの中身がそれを超えるときだけ、
//! 50トークンを重ねて分ける。トークンは、Python版と同じtiktokenの `cl100k_base` で数える。

use regex::Regex;
use std::sync::LazyLock;
use tiktoken_rs::{CoreBPE, cl100k_base_singleton};

use crate::text::{py_splitlines, py_strip};

pub const CHUNK_TARGET_TOKENS: usize = 500;
pub const OVERLAP_TOKENS: usize = 50;
pub const NO_HEADING: &str = "(見出しなし)";

static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.*)$").unwrap());

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub heading: String,
    pub text: String,
}

fn bpe() -> &'static CoreBPE {
    cl100k_base_singleton()
}

fn split_by_heading(markdown: &str) -> Vec<Chunk> {
    let mut sections = Vec::new();
    let mut heading = NO_HEADING.to_string();
    let mut lines: Vec<&str> = Vec::new();
    let flush = |heading: &str, lines: &[&str], out: &mut Vec<Chunk>| {
        let content = lines.join("\n");
        let content = py_strip(&content);
        if !content.is_empty() {
            out.push(Chunk { heading: heading.to_string(), text: content.to_string() });
        }
    };
    for line in py_splitlines(markdown) {
        if let Some(c) = HEADING.captures(line) {
            flush(&heading, &lines, &mut sections);
            heading = py_strip(c.get(2).map_or("", |m| m.as_str())).to_string();
            lines.clear();
        } else {
            lines.push(line);
        }
    }
    flush(&heading, &lines, &mut sections);
    sections
}

fn split_long_section(section: Chunk) -> Vec<Chunk> {
    let bpe = bpe();
    let tokens = bpe.encode_ordinary(&section.text);
    if tokens.len() <= CHUNK_TARGET_TOKENS {
        return vec![section];
    }
    let mut parts = Vec::new();
    let step = CHUNK_TARGET_TOKENS - OVERLAP_TOKENS;
    let mut start = 0;
    while start < tokens.len() {
        let end = (start + CHUNK_TARGET_TOKENS).min(tokens.len());
        // Pythonの tiktoken の decode と同じく、トークンの境目で切れた文字は置き換え文字にする
        let bytes = bpe.decode_bytes(&tokens[start..end]).unwrap_or_default();
        parts.push(Chunk { heading: section.heading.clone(), text: String::from_utf8_lossy(&bytes).into_owned() });
        if start + CHUNK_TARGET_TOKENS >= tokens.len() {
            break;
        }
        start += step;
    }
    parts
}

/// 1つの区間(見出し+本文)を、長ければ重ねて分ける。素材から抽出した文に使う。
pub fn split_section(section: Chunk) -> Vec<Chunk> {
    split_long_section(section)
}

/// 見出し単位で分け、長すぎる区間はさらに分ける。
pub fn chunk_markdown(markdown: &str) -> Vec<Chunk> {
    split_by_heading(markdown).into_iter().flat_map(split_long_section).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 見出しで分ける() {
        let md = "---\ntitle: x\n---\n前書き\n\n## 背景\n本文1\n\n### 細部 \n本文2\n## 空\n\n";
        let c = chunk_markdown(md);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0], Chunk { heading: NO_HEADING.into(), text: "---\ntitle: x\n---\n前書き".into() });
        assert_eq!(c[1].heading, "背景");
        assert_eq!(c[2], Chunk { heading: "細部".into(), text: "本文2".into() });
    }

    #[test]
    fn 長い区間は重ねて分ける() {
        let text = "詩織は図書館の司書です。".repeat(200);
        let n = bpe().encode_ordinary(&text).len();
        let parts = split_section(Chunk { heading: "h".into(), text });
        let expected = (n - OVERLAP_TOKENS).div_ceil(CHUNK_TARGET_TOKENS - OVERLAP_TOKENS);
        assert_eq!(parts.len(), expected);
        assert!(parts.iter().all(|p| p.heading == "h"));
    }

    #[test]
    fn 見出しの記号だけの行は見出しではない() {
        let c = chunk_markdown("#\n本文\n#タグ\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "#\n本文\n#タグ");
    }
}
