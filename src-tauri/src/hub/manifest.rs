//! 拡張機能の説明書(`extension.json`)の読み込みと確かめ(詩織Ver6.0の段階1)。
//!
//! 説明書は、ふらるとAIが手で書く。書き間違い(項目名の打ち間違いなど)を黙って見逃さないよう、
//! 知らない項目があれば読み込まない(`deny_unknown_fields`)。読めない拡張機能は、その1つだけを
//! 読み込まず、エラーコード付きで知らせる(ほかの拡張機能と詩織は止めない)。

use crate::error_codes::{self, ErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

pub const MANIFEST_FILE: &str = "extension.json";

/// 詩織が拡張機能に貸すもの(構想メモの「貸すもの」)。名前と、承認の画面に出す説明。
/// ログとエラーの報告は全員に貸すので、ここには無い(`uses`に書かなくてよい)。
pub const USES: &[(&str, &str)] = &[
    ("notify", "知らせ(ペットの吹き出し・ウィンドウの知らせ)"),
    ("schedule", "時刻の予約(時間が来たら詩織が知らせる)"),
    ("storage", "設定と状態の保存(この拡張機能の置き場の中だけ)"),
    ("clipboard.write", "クリップボードへの書き込み"),
    ("library.read", "library・棚・memoryを読む(検索と閲覧だけ。書き込みはできない)"),
    ("llm.ask", "ローカルのLLMへの質問"),
    ("ext.call", "ほかの拡張機能を呼ぶ(詩織を通して、相手が許した操作だけ)"),
];

pub fn use_label(name: &str) -> Option<&'static str> {
    USES.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// 画面の部品(HTMLとJavaScript)。詩織の画面の中の囲いで動く
    Ui,
    /// 別のプログラム。詩織が起動し、標準入出力でやり取りする
    Program,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    Window,
    Pet,
}

/// 始める場所。画面の部品はHTMLのファイル、別のプログラムはOSごとの実行ファイル。
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum Entry {
    File(String),
    PerOs {
        #[serde(default)]
        win: Option<String>,
        #[serde(default)]
        mac: Option<String>,
    },
}

impl Entry {
    /// このOSで使う入口のファイル(拡張機能のフォルダからの相対パス)。
    pub fn for_this_os(&self) -> Option<&str> {
        match self {
            Entry::File(path) => Some(path),
            Entry::PerOs { win, mac } => {
                if cfg!(windows) {
                    win.as_deref()
                } else if cfg!(target_os = "macos") {
                    mac.as_deref()
                } else {
                    None
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Provide {
    pub method: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ErrorDef {
    pub code: String,
    pub message: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub kind: Kind,
    pub entry: Entry,
    pub places: Vec<Place>,
    #[serde(default)]
    pub uses: Vec<String>,
    #[serde(default)]
    pub provides: Vec<Provide>,
    #[serde(default)]
    pub errors: Vec<ErrorDef>,
}

/// 読み込みの失敗。エラーコードと、何が悪いかの説明。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    pub code: ErrorCode,
    pub detail: String,
}

impl ManifestError {
    fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self { code, detail: detail.into() }
    }
}

/// 拡張機能のフォルダ1つから説明書を読み、確かめる。
pub fn load(dir: &Path) -> Result<Manifest, ManifestError> {
    let path = dir.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| ManifestError::new(error_codes::EXT_001, format!("{} ({e})", path.display())))?;
    let manifest: Manifest = serde_json::from_str(&text)
        .map_err(|e| ManifestError::new(error_codes::EXT_002, e.to_string()))?;
    let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    validate(&manifest, &folder, dir)?;
    Ok(manifest)
}

/// 名前に使える文字: 英小文字・数字・ハイフン。先頭は英小文字か数字。64文字まで。
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    name.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_valid_method(method: &str) -> bool {
    let mut chars = method.chars();
    let Some(first) = chars.next() else { return false };
    first.is_ascii_alphabetic() && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// 拡張機能のフォルダの中を指す相対パスか(`..`・絶対パス・ドライブ名を断る)。
pub fn is_inside_relative(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn check_file(dir: &Path, path: &str, what: &str) -> Result<(), ManifestError> {
    if !is_inside_relative(path) {
        return Err(ManifestError::new(error_codes::EXT_005, format!("{what}がフォルダの外を指している: {path}")));
    }
    if !dir.join(path).is_file() {
        return Err(ManifestError::new(error_codes::EXT_005, format!("{what}のファイルが無い: {path}")));
    }
    Ok(())
}

pub fn validate(m: &Manifest, folder: &str, dir: &Path) -> Result<(), ManifestError> {
    let bad = |detail: String| Err(ManifestError::new(error_codes::EXT_004, detail));
    if !is_valid_name(&m.name) {
        return bad(format!("nameに使えない文字がある(英小文字・数字・ハイフン): {}", m.name));
    }
    if m.name != folder {
        return Err(ManifestError::new(
            error_codes::EXT_003,
            format!("name={} フォルダ={folder}", m.name),
        ));
    }
    if m.version.trim().is_empty() {
        return bad("versionが空".into());
    }
    if m.title.trim().is_empty() {
        return bad("titleが空".into());
    }
    if m.places.is_empty() {
        return bad("placesが空(window・petから選ぶ)".into());
    }
    match (m.kind, &m.entry) {
        (Kind::Ui, Entry::File(_)) => {}
        (Kind::Ui, Entry::PerOs { .. }) => return bad("kind=uiのentryは、HTMLのファイルの名前".into()),
        (Kind::Program, _) => {}
    }
    let entry = m
        .entry
        .for_this_os()
        .ok_or_else(|| ManifestError::new(error_codes::EXT_004, "このOSのentryが無い"))?;
    check_file(dir, entry, "entry")?;
    if let Some(icon) = &m.icon {
        check_file(dir, icon, "icon")?;
    }
    let mut seen = HashSet::new();
    for u in &m.uses {
        if use_label(u).is_none() {
            return Err(ManifestError::new(error_codes::EXT_006, format!("知らない使うもの: {u}")));
        }
        if !seen.insert(u.as_str()) {
            return bad(format!("usesが重なっている: {u}"));
        }
    }
    let mut seen = HashSet::new();
    for p in &m.provides {
        if !is_valid_method(&p.method) {
            return bad(format!("providesのmethodに使えない文字がある: {}", p.method));
        }
        if !seen.insert(p.method.as_str()) {
            return bad(format!("providesが重なっている: {}", p.method));
        }
    }
    let prefix = format!("{}-", m.name);
    let mut seen = HashSet::new();
    for e in &m.errors {
        let num = e.code.strip_prefix(&prefix).unwrap_or("");
        if num.len() != 3 || !num.chars().all(|c| c.is_ascii_digit()) {
            return bad(format!("errorsのcodeは「{prefix}3桁の番号」の形にする: {}", e.code));
        }
        if !seen.insert(e.code.as_str()) {
            return bad(format!("errorsのcodeが重なっている: {}", e.code));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;

    /// テストごとに空の作業用フォルダを作る(終わったら消す)。
    pub(crate) struct TempDir(pub PathBuf);
    impl TempDir {
        pub(crate) fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "shiori-test-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 拡張機能のフォルダを1つ作る(説明書と入口のファイル)。
    pub(crate) fn make_ext(base: &Path, folder: &str, manifest: &str) -> PathBuf {
        let dir = base.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(MANIFEST_FILE), manifest).unwrap();
        std::fs::write(dir.join("index.html"), "<p>hi</p>").unwrap();
        dir
    }

    pub(crate) fn ui_manifest(name: &str, uses: &str) -> String {
        format!(
            r#"{{"name": "{name}", "version": "0.1.0", "title": "試し", "kind": "ui",
               "entry": "index.html", "places": ["window"], "uses": [{uses}],
               "provides": [{{"method": "greet"}}], "errors": [{{"code": "{name}-001", "message": "空"}}]}}"#
        )
    }

    #[test]
    fn 正しい説明書を読める() {
        let t = TempDir::new("manifest-ok");
        let dir = make_ext(&t.0, "hello-ui", &ui_manifest("hello-ui", r#""storage", "ext.call""#));
        let m = load(&dir).unwrap();
        assert_eq!(m.kind, Kind::Ui);
        assert_eq!(m.uses, vec!["storage", "ext.call"]);
        assert_eq!(m.entry.for_this_os(), Some("index.html"));
    }

    #[test]
    fn 読めない説明書はエラーコードで知らせる() {
        let t = TempDir::new("manifest-bad");
        let code = |folder: &str, manifest: &str| load(&make_ext(&t.0, folder, manifest)).unwrap_err().code.code;
        // 説明書が無い
        std::fs::create_dir_all(t.0.join("empty")).unwrap();
        assert_eq!(load(&t.0.join("empty")).unwrap_err().code.code, "EXT-001");
        // 知らない項目(打ち間違い)
        let typo = ui_manifest("a", "").replace("\"uses\"", "\"use\"");
        assert_eq!(code("a", &typo), "EXT-002");
        // 名前がフォルダと違う
        assert_eq!(code("b", &ui_manifest("other", "")), "EXT-003");
        // 名前に使えない文字
        assert_eq!(code("Big", &ui_manifest("Big", "")), "EXT-004");
        // 入口がフォルダの外
        let outside = ui_manifest("c", "").replace("\"index.html\"", "\"../index.html\"");
        assert_eq!(code("c", &outside), "EXT-005");
        // 貸さないもの
        assert_eq!(code("d", &ui_manifest("d", r#""network""#)), "EXT-006");
        // エラーコードの形
        let bad_code = ui_manifest("e", "").replace("e-001", "x-001");
        assert_eq!(code("e", &bad_code), "EXT-004");
        // 画面の部品にOSごとのentry
        let per_os = ui_manifest("f", "").replace("\"index.html\"", r#"{"win": "index.html"}"#);
        assert_eq!(code("f", &per_os), "EXT-004");
    }

    #[test]
    fn フォルダの中を指すかの判定() {
        assert!(is_inside_relative("index.html"));
        assert!(is_inside_relative("bin/x.exe"));
        assert!(is_inside_relative("./a/b.js"));
        assert!(!is_inside_relative("../x"));
        assert!(!is_inside_relative("a/../../x"));
        assert!(!is_inside_relative(""));
        assert!(!is_inside_relative(if cfg!(windows) { "C:\\x" } else { "/x" }));
    }

    #[test]
    fn 名前の判定() {
        assert!(is_valid_name("hello-ui"));
        assert!(is_valid_name("calc2"));
        assert!(!is_valid_name("-a"));
        assert!(!is_valid_name("Hello"));
        assert!(!is_valid_name("a_b"));
        assert!(!is_valid_name(""));
    }
}
