//! 形式ごとの抽出(`services/rag/extraction/formats.py` と同じ手順)。
//!
//! 抽出の関数は、(見出し, 本文)の並びを返す。読むだけで、マクロ・JavaScript・埋め込みファイルは
//! 実行も展開もしない。素材は信頼できない入力なので、このファイルの関数は別プロセス
//! (`shiori-rag extract`)の中でだけ呼ぶ。

use std::io::Read;
use std::path::Path;

use regex::Regex;

use super::limits;

pub type Section = (String, String);

/// 抽出できない理由が分かっている失敗。`reason` は記録・警告にそのまま出す。
#[derive(Debug)]
pub struct ExtractionError {
    pub reason: String,
    pub detail: String,
}

impl ExtractionError {
    pub fn new(reason: &str) -> Self {
        Self { reason: reason.into(), detail: String::new() }
    }
    pub fn with(reason: &str, detail: impl ToString) -> Self {
        Self { reason: reason.into(), detail: detail.to_string() }
    }
}

type R<T> = Result<T, ExtractionError>;

const TEXT_EXTS: &[&str] =
    &["txt", "text", "log", "csv", "tsv", "json", "yaml", "yml", "xml", "ini", "toml", "rst", "md"];

/// 拡張子(小文字、ドットなし)に対応する抽出があるか。
pub fn supported(path: &Path) -> bool {
    extractor_for(&ext_of(path)).is_some()
}

pub fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

pub fn extractor_for(ext: &str) -> Option<fn(&Path) -> R<Vec<Section>>> {
    Some(match ext {
        "pdf" => extract_pdf,
        "docx" => extract_docx,
        "pptx" => extract_pptx,
        "xlsx" => extract_xlsx,
        "html" | "htm" => extract_html,
        e if TEXT_EXTS.contains(&e) => extract_text_file,
        _ => return None,
    })
}

fn read(path: &Path) -> R<Vec<u8>> {
    std::fs::read(path).map_err(|e| ExtractionError::with("抽出に失敗", e))
}

// ---------- テキスト系 ----------

/// Python版と同じ順(UTF-8〈BOM付きも〉→ cp932 → UTF-16)で、厳密に読めたもので読む。
pub fn decode_text(data: &[u8]) -> R<String> {
    let head = &data[..data.len().min(4096)];
    if head.contains(&0) && !data.starts_with(b"\xff\xfe") && !data.starts_with(b"\xfe\xff") {
        return Err(ExtractionError::new("テキストではない(バイナリ)"));
    }
    let utf8 = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    if let Ok(s) = std::str::from_utf8(utf8) {
        return Ok(s.to_string());
    }
    if let Some(s) = encoding_rs::SHIFT_JIS.decode_without_bom_handling_and_without_replacement(data) {
        return Ok(s.into_owned());
    }
    if let Some(s) = decode_utf16(data) {
        return Ok(s);
    }
    Ok(String::from_utf8_lossy(data).into_owned())
}

/// Pythonの "utf-16"(BOMがあればそれに従い、無ければリトルエンディアン。BOMは外す)。
fn decode_utf16(data: &[u8]) -> Option<String> {
    let (enc, body) = if let Some(b) = data.strip_prefix(b"\xff\xfe") {
        (encoding_rs::UTF_16LE, b)
    } else if let Some(b) = data.strip_prefix(b"\xfe\xff") {
        (encoding_rs::UTF_16BE, b)
    } else {
        (encoding_rs::UTF_16LE, data)
    };
    if body.len() % 2 != 0 {
        return None;
    }
    enc.decode_without_bom_handling_and_without_replacement(body).map(|s| s.into_owned())
}

fn extract_text_file(path: &Path) -> R<Vec<Section>> {
    Ok(vec![("本文".into(), decode_text(&read(path)?)?)])
}

// ---------- HTML ----------

mod html {
    use std::cell::{Cell, RefCell};

    use html5ever::tendril::StrTendril;
    use html5ever::tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts, states::RawKind,
    };

    const SKIP: &[&str] = &["script", "style", "noscript", "template", "head"];
    const BLOCK: &[&str] =
        &["p", "div", "br", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6", "section", "article", "table"];

    /// Python版(`html.parser.HTMLParser` を使った `_HtmlText`)と同じく、タグの木は作らず、
    /// タグと文字の並びだけを見る。script・style・head などの中は捨て、段落のタグで改行を入れる。
    #[derive(Default)]
    struct Sink {
        parts: RefCell<Vec<String>>,
        skip_depth: Cell<usize>,
    }

    impl Sink {
        fn start(&self, name: &str) {
            if SKIP.contains(&name) {
                self.skip_depth.set(self.skip_depth.get() + 1);
            } else if BLOCK.contains(&name) {
                self.parts.borrow_mut().push("\n".into());
            }
        }
        fn end(&self, name: &str) {
            if SKIP.contains(&name) && self.skip_depth.get() > 0 {
                self.skip_depth.set(self.skip_depth.get() - 1);
            } else if BLOCK.contains(&name) {
                self.parts.borrow_mut().push("\n".into());
            }
        }
    }

    impl TokenSink for Sink {
        type Handle = ();

        fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
            match token {
                Token::TagToken(tag) => {
                    let name = tag.name.to_string();
                    match tag.kind {
                        TagKind::StartTag => {
                            self.start(&name);
                            if tag.self_closing {
                                self.end(&name);
                            } else if name == "script" {
                                return TokenSinkResult::RawData(RawKind::ScriptData);
                            } else if name == "style" {
                                return TokenSinkResult::RawData(RawKind::Rawtext);
                            }
                        }
                        TagKind::EndTag => self.end(&name),
                    }
                }
                Token::CharacterTokens(t) if self.skip_depth.get() == 0 => self.parts.borrow_mut().push(t.to_string()),
                _ => {}
            }
            TokenSinkResult::Continue
        }
    }

    pub fn text_of(input: &str) -> String {
        let tok = Tokenizer::new(Sink::default(), TokenizerOpts::default());
        let queue = BufferQueue::default();
        queue.push_back(StrTendril::from_slice(input));
        let _ = tok.feed(&queue);
        tok.end();
        tok.sink.parts.take().concat()
    }
}

fn extract_html(path: &Path) -> R<Vec<Section>> {
    let raw = html::text_of(&decode_text(&read(path)?)?);
    let re = Regex::new(r"\n\s*\n+").unwrap();
    let text = re.replace_all(&raw, "\n\n");
    Ok(vec![("本文".into(), crate::text::py_strip(&text).to_string())])
}

// ---------- PDF ----------

fn extract_pdf(path: &Path) -> R<Vec<Section>> {
    let bytes = read(path)?;
    let doc = lopdf::Document::load_mem(&bytes).map_err(|e| {
        let msg = e.to_string();
        // 暗号化PDFは、空のパスワードで開けるものも含めて扱わない(パスワードは試さない)
        if msg.to_lowercase().contains("encrypt") || msg.to_lowercase().contains("decrypt") {
            ExtractionError::new("暗号化されたPDF")
        } else {
            ExtractionError::with("壊れたPDF", msg.chars().take(100).collect::<String>())
        }
    })?;
    if doc.is_encrypted() || doc.trailer.get(b"Encrypt").is_ok() {
        return Err(ExtractionError::new("暗号化されたPDF"));
    }
    let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes)
        .map_err(|e| ExtractionError::with("壊れたPDF", e.to_string().chars().take(100).collect::<String>()))?;
    Ok(pages
        .into_iter()
        .take(limits::MAX_PAGES)
        .enumerate()
        .filter_map(|(i, t)| {
            let t = crate::text::py_strip(&t).to_string();
            (!t.is_empty()).then(|| (format!("p.{}", i + 1), t))
        })
        .collect())
}

// ---------- OOXML(docx / pptx / xlsx。中身はzip+XML) ----------

/// XMLの小さな木。Pythonの ElementTree と同じく、要素の名前(名前空間を外したもの)・属性・
/// 開始タグの直後の文字(`.text`)・子を持つ。
#[derive(Debug, Default)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: Option<String>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// 自分と子孫を、文書の順に並べる(ElementTreeの `iter()`)。
    pub fn iter(&self) -> Vec<&Node> {
        let mut out = vec![self];
        for c in &self.children {
            out.extend(c.iter());
        }
        out
    }

    /// 子孫の `t` 要素の文字をつなげる(自分も含む)。
    fn t_text(&self) -> String {
        self.iter().into_iter().filter(|n| n.name == "t").map(|n| n.text.as_deref().unwrap_or("")).collect()
    }
}

fn local(name: &str) -> String {
    name.rsplit(':').next().unwrap_or("").to_string()
}

pub fn parse_xml(data: &[u8], name: &str) -> R<Node> {
    use quick_xml::events::Event;
    let bad = |e: &dyn std::fmt::Display| ExtractionError::with("壊れたXML", format!("{name}: {e}"));
    // 外部の実体の展開(XXE)や、実体の入れ子での膨張は扱わない(Python版のdefusedxmlと同じく断る)
    if data.windows(8).any(|w| w == b"<!ENTITY") {
        return Err(ExtractionError::with("不正なXML", format!("{name}: EntitiesForbidden")));
    }
    let mut reader = quick_xml::Reader::from_reader(data);
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut buf = Vec::new();
    loop {
        let ev = reader.read_event_into(&mut buf).map_err(|e| bad(&e))?;
        match ev {
            Event::Start(e) => {
                let node = element(&e).map_err(|e| bad(&e))?;
                stack.push(node);
            }
            Event::Empty(e) => {
                let node = element(&e).map_err(|e| bad(&e))?;
                stack.last_mut().unwrap().children.push(node);
            }
            Event::End(_) => {
                let node = stack.pop().ok_or_else(|| bad(&"閉じタグが多い"))?;
                stack.last_mut().ok_or_else(|| bad(&"閉じタグが多い"))?.children.push(node);
            }
            Event::Text(t) => {
                let s = t.xml10_content().into_owned();
                push_text(stack.last_mut().unwrap(), &s);
            }
            Event::GeneralRef(r) => {
                let s = resolve_ref(r).ok_or_else(|| bad(&"知らない実体の参照"))?;
                push_text(stack.last_mut().unwrap(), &s);
            }
            Event::CData(c) => {
                let s = c.into_inner().into_owned();
                push_text(stack.last_mut().unwrap(), &s);
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    if stack.len() != 1 {
        return Err(bad(&"閉じていない要素がある"));
    }
    let mut root = stack.pop().unwrap();
    if root.children.len() != 1 {
        return Err(bad(&"根の要素が1つではない"));
    }
    Ok(root.children.pop().unwrap())
}

/// 開始タグの直後の文字だけを `.text` に持つ(子の後ろの文字〈tail〉は、この抽出では使わない)。
fn push_text(node: &mut Node, s: &str) {
    if node.children.is_empty() {
        node.text.get_or_insert_with(String::new).push_str(s);
    }
}

fn resolve_ref(r: quick_xml::events::BytesRef) -> Option<String> {
    let name = r.into_inner().into_owned();
    let named = match name.as_str() {
        "lt" => "<",
        "gt" => ">",
        "amp" => "&",
        "apos" => "'",
        "quot" => "\"",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }
    let code = if let Some(h) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        u32::from_str_radix(h, 16).ok()?
    } else {
        name.strip_prefix('#')?.parse().ok()?
    };
    char::from_u32(code).map(String::from)
}

fn element(e: &quick_xml::events::BytesStart) -> Result<Node, quick_xml::Error> {
    let mut attrs = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(quick_xml::Error::InvalidAttr)?;
        let value = a.normalized_value(quick_xml::XmlVersion::Implicit1_0)?.into_owned();
        attrs.push((a.key.as_ref().to_string(), value));
    }
    Ok(Node { name: local(e.name().as_ref()), attrs, text: None, children: Vec::new() })
}

struct Zip {
    archive: zip::ZipArchive<std::fs::File>,
}

impl Zip {
    /// zip爆弾(展開後が巨大・圧縮率が異常)を、展開する前に弾いてから開く。
    fn open(path: &Path) -> R<Self> {
        let f = std::fs::File::open(path).map_err(|e| ExtractionError::with("抽出に失敗", e))?;
        let mut archive =
            zip::ZipArchive::new(f).map_err(|_| ExtractionError::new("壊れたファイル(zipとして読めない)"))?;
        if archive.len() > limits::ZIP_MAX_MEMBERS {
            return Err(ExtractionError::new("zipの中のファイルが多すぎる"));
        }
        let mut total = 0u64;
        for i in 0..archive.len() {
            let m = archive.by_index_raw(i).map_err(|_| ExtractionError::new("壊れたファイル(zipとして読めない)"))?;
            let (size, comp) = (m.size(), m.compressed_size());
            total += size;
            if size > limits::ZIP_MAX_MEMBER_BYTES || total > limits::ZIP_MAX_TOTAL_BYTES {
                return Err(ExtractionError::new("zipの展開サイズが大きすぎる"));
            }
            if comp > 0 && size as f64 / comp as f64 > limits::ZIP_MAX_RATIO {
                return Err(ExtractionError::new("zipの圧縮率が異常(zip爆弾の疑い)"));
            }
        }
        Ok(Self { archive })
    }

    fn names(&self) -> Vec<String> {
        self.archive.file_names().filter_map(|n| n.ok()).map(|n| n.into_owned()).collect()
    }

    fn xml(&mut self, name: &str) -> R<Node> {
        let mut f = self.archive.by_name(name).map_err(|_| ExtractionError::with("必要な部品が無い", name))?;
        let mut data = Vec::new();
        // 展開の大きさは開いたときに確かめているが、念のため上限で止める
        (&mut f)
            .take(limits::ZIP_MAX_MEMBER_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|e| ExtractionError::with("壊れたファイル(zipとして読めない)", e))?;
        parse_xml(&data, name)
    }

    /// `prefix` で始まり `.xml` で終わる部品を、名前の末尾の番号の順に並べる(slide10はslide9の後)。
    fn numbered(&self, prefix: &str) -> Vec<String> {
        let re = Regex::new(r"(\d+)\.xml$").unwrap();
        let mut names: Vec<String> =
            self.names().into_iter().filter(|n| n.starts_with(prefix) && n.ends_with(".xml")).collect();
        let key = |n: &str| re.captures(n).and_then(|c| c[1].parse::<u64>().ok()).unwrap_or(0);
        names.sort_by_key(|n| key(n));
        names
    }
}

fn paragraphs(root: &Node) -> Vec<String> {
    root.iter().into_iter().filter(|n| n.name == "p").map(Node::t_text).filter(|t| !t.trim().is_empty()).collect()
}

fn extract_docx(path: &Path) -> R<Vec<Section>> {
    let mut z = Zip::open(path)?;
    let root = z.xml("word/document.xml")?;
    Ok(vec![("本文".into(), paragraphs(&root).join("\n"))])
}

fn extract_pptx(path: &Path) -> R<Vec<Section>> {
    let mut z = Zip::open(path)?;
    let mut sections = Vec::new();
    for (i, name) in z.numbered("ppt/slides/slide").into_iter().enumerate() {
        if i >= limits::MAX_PAGES {
            break;
        }
        let lines = paragraphs(&z.xml(&name)?);
        if !lines.is_empty() {
            sections.push((format!("スライド{}", i + 1), lines.join("\n")));
        }
    }
    Ok(sections)
}

fn extract_xlsx(path: &Path) -> R<Vec<Section>> {
    let mut z = Zip::open(path)?;
    let names = z.names();
    let mut shared = Vec::new();
    if names.iter().any(|n| n == "xl/sharedStrings.xml") {
        for si in z.xml("xl/sharedStrings.xml")?.children {
            shared.push(si.t_text());
        }
    }
    let mut sheet_names = Vec::new();
    if names.iter().any(|n| n == "xl/workbook.xml") {
        let wb = z.xml("xl/workbook.xml")?;
        sheet_names = wb.iter().into_iter().filter(|n| n.name == "sheet").map(|n| n.get("name").unwrap_or("").to_string()).collect();
    }
    let mut sections = Vec::new();
    for (i, name) in z.numbered("xl/worksheets/sheet").into_iter().enumerate() {
        if i >= limits::MAX_PAGES {
            break;
        }
        let root = z.xml(&name)?;
        let mut rows = Vec::new();
        for row in root.iter().into_iter().filter(|n| n.name == "row") {
            let mut cells = Vec::new();
            for c in row.children.iter().filter(|n| n.name == "c") {
                let v = c.children.iter().find(|x| x.name == "v");
                let v_text = v.and_then(|v| v.text.as_deref()).filter(|t| !t.is_empty());
                let shared_idx = v_text
                    .filter(|t| t.chars().all(|ch| ch.is_ascii_digit()))
                    .and_then(|t| t.parse::<usize>().ok())
                    .filter(|&i| i < shared.len());
                if c.get("t") == Some("s") && shared_idx.is_some() {
                    cells.push(shared[shared_idx.unwrap()].clone());
                } else if c.get("t") == Some("inlineStr") {
                    cells.push(c.t_text());
                } else if let Some(t) = v_text {
                    cells.push(t.to_string());
                }
            }
            if !cells.is_empty() {
                rows.push(cells.join("\t"));
            }
        }
        if !rows.is_empty() {
            let label = match sheet_names.get(i) {
                Some(n) if !n.is_empty() => n.clone(),
                _ => format!("シート{}", i + 1),
            };
            sections.push((label, rows.join("\n")));
        }
    }
    Ok(sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 文字コード() {
        assert_eq!(decode_text("日本語".as_bytes()).unwrap(), "日本語");
        assert_eq!(decode_text(b"\xef\xbb\xbfabc").unwrap(), "abc");
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("日本語");
        assert_eq!(decode_text(&sjis).unwrap(), "日本語");
        let mut u16 = vec![0xff, 0xfe];
        for u in "日本".encode_utf16() {
            u16.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_text(&u16).unwrap(), "日本");
        assert_eq!(decode_text(b"a\x00b").unwrap_err().reason, "テキストではない(バイナリ)");
    }

    #[test]
    fn htmlの文字() {
        let t = html::text_of(
            "<html><head><title>題</title><style>p{}</style></head><body><p>一&amp;二</p><script>if(a<b){}</script><br/>三<div>四</div></body></html>",
        );
        let re = Regex::new(r"\n\s*\n+").unwrap();
        assert_eq!(crate::text::py_strip(&re.replace_all(&t, "\n\n")), "一&二\n\n三\n四");
    }

    #[test]
    fn xmlの木() {
        let root = parse_xml(
            "<w:document xmlns:w=\"x\"><w:body><w:p><w:r><w:t>こん</w:t></w:r><w:r><w:t xml:space=\"preserve\">にちは &amp; </w:t></w:r></w:p><w:p/></w:body></w:document>".as_bytes(),
            "doc",
        )
        .unwrap();
        assert_eq!(paragraphs(&root), vec!["こんにちは & "]);
        assert!(parse_xml(b"<!DOCTYPE x [<!ENTITY a \"b\">]><x>&a;</x>", "x").is_err());
        assert!(parse_xml(b"<x><y></x>", "x").is_err());
    }
}
