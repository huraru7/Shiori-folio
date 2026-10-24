//! 詩織の診断のCLI(詩織Ver6.0の段階6)。窓を使わないので、本体の画面が出ないPCでも動く。
//!
//! 使用法: `shiori-doctor`               … 診断の文章を、画面・`data/logs/diagnose-日時.txt`・クリップボードに出す
//!         `shiori-doctor --no-copy`     … クリップボードに写さない
//!         `shiori-doctor --no-save`     … ファイルに書かない
//!         `shiori-doctor --root <パス>` … SSD(portable/)のルートを指定する(省略時は実行ファイルの場所から探す)
//!
//! SSD直下の `診断.command`(Mac)・`診断.bat`(Windows)から開く想定。何も書き換えない
//! (書くのは診断のファイルだけ)。終了コード: 0=NGなし、1=NGがある、2=引数の誤り。

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use shiori_folio_lib::diagnose;
use shiori_folio_lib::project_root;

fn main() {
    let mut copy = true;
    let mut save = true;
    let mut root: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--no-copy" => copy = false,
            "--no-save" => save = false,
            "--root" => match args.next() {
                Some(p) => root = Some(PathBuf::from(p)),
                None => usage_exit("--rootにはパスが必要です"),
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other => usage_exit(&format!("不明な引数: {other}")),
        }
    }
    let root = root.unwrap_or_else(project_root);

    let report = diagnose::build(&root, "shiori-doctor");
    println!("{}", report.text);

    if save {
        match diagnose::save(&root, &report) {
            Ok(path) => println!("診断をファイルに書きました: {}", path.display()),
            Err(e) => eprintln!("診断をファイルに書けませんでした(SSDに書き込めないのかもしれません): {e}"),
        }
    }
    if copy {
        match copy_to_clipboard(&report.text) {
            Ok(()) => println!("クリップボードに写しました。外部のAIにそのまま貼り付けられます。"),
            Err(e) => eprintln!("クリップボードに写せませんでした: {e}"),
        }
    }
    std::process::exit(if report.ng > 0 { 1 } else { 0 });
}

const USAGE: &str = "使い方: shiori-doctor [--no-copy] [--no-save] [--root <portable/のパス>]";

fn usage_exit(msg: &str) -> ! {
    eprintln!("{msg}\n{USAGE}");
    std::process::exit(2);
}

/// OSの道具でクリップボードに写す(窓を使わないため、Tauriのクリップボードは使えない)。
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let (program, bytes): (&str, Vec<u8>) = if cfg!(target_os = "macos") {
        ("pbcopy", text.as_bytes().to_vec())
    } else if cfg!(windows) {
        // clip.exe はUTF-8を正しく読まないので、BOM付きのUTF-16LEで渡す
        let mut b = vec![0xFF, 0xFE];
        b.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        ("clip", b)
    } else {
        return Err("このOSでは写せない".into());
    };
    let mut child = Command::new(program)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}を起動できない({e})"))?;
    child
        .stdin
        .take()
        .ok_or("標準入力を開けない")?
        .write_all(&bytes)
        .map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("{program}が失敗した({status})")) }
}
