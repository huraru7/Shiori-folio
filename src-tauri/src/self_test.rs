//! 自己確認の起動(詩織Ver6.0の段階2)。
//!
//! `shiori-folio.exe --self-test` で起動すると、画面の側が列の項目を全部開いて確かめ、
//! 1項目1行で `[SELFTEST]` のログに書き、最後に本体を閉じる。ふらるが画面を見なくても、
//! Claudeがログで確かめられるようにするための仕組み。ふつうの起動では動かない。
//!
//! - `--self-test-break=<項目のid>`: その画面をわざと壊し、`NG` になることを確かめる。
//! - 画面の側が止まっても残らないよう、決まった時間で打ち切って閉じる。

use crate::shiori_log;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;

/// 打ち切るまでの時間(起動画面でのモデルの読み込みを含む)
const TIMEOUT: Duration = Duration::from_secs(300);

const PART: &str = "SELFTEST";

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SelfTestMode {
    pub enabled: bool,
    /// わざと壊す画面のid
    pub break_target: Option<String>,
}

fn mode() -> &'static SelfTestMode {
    static MODE: OnceLock<SelfTestMode> = OnceLock::new();
    MODE.get_or_init(|| from_args(std::env::args()))
}

fn from_args(args: impl Iterator<Item = String>) -> SelfTestMode {
    let mut m = SelfTestMode::default();
    for arg in args {
        if arg == "--self-test" {
            m.enabled = true;
        } else if let Some(target) = arg.strip_prefix("--self-test-break=") {
            m.break_target = Some(target.to_string());
        }
    }
    if !m.enabled {
        m.break_target = None;
    }
    m
}

pub fn is_enabled() -> bool {
    mode().enabled
}

/// 自己確認の起動なら、打ち切りの見張りを始める。
pub fn start_watchdog(app: tauri::AppHandle) {
    if !is_enabled() {
        return;
    }
    shiori_log::info(PART, "自己確認の起動で始めた", &[("壊す画面", &mode().break_target.as_deref().unwrap_or("なし"))]);
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        shiori_log::error(PART, "時間内に終わらなかったので打ち切った", &[("秒", &TIMEOUT.as_secs())]);
        app.exit(2);
    });
}

#[tauri::command]
pub fn self_test_mode() -> SelfTestMode {
    mode().clone()
}

/// 1項目の結果を書く。`result`は "OK" か "NG"。NGは「エラー」の重さで書く。
#[tauri::command]
pub fn self_test_log(item: String, result: String, fields: Option<BTreeMap<String, String>>) {
    if !is_enabled() {
        return;
    }
    let fields = fields.unwrap_or_default();
    let mut pairs: Vec<(&str, &dyn std::fmt::Display)> = vec![("項目", &item), ("結果", &result)];
    pairs.extend(fields.iter().map(|(k, v)| (k.as_str(), v as &dyn std::fmt::Display)));
    if result == "OK" {
        shiori_log::info(PART, "確かめた", &pairs);
    } else {
        shiori_log::error(PART, "確かめた", &pairs);
    }
}

/// 終わりの行を書いて、本体を閉じる。
#[tauri::command]
pub fn self_test_finish(app: tauri::AppHandle, ok: u32, ng: u32) {
    if !is_enabled() {
        return;
    }
    let pairs: [(&str, &dyn std::fmt::Display); 2] = [("OK", &ok), ("NG", &ng)];
    if ng == 0 {
        shiori_log::info(PART, "終わった", &pairs);
    } else {
        shiori_log::error(PART, "終わった", &pairs);
    }
    app.exit(if ng == 0 { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> SelfTestMode {
        from_args(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn 引数で自己確認の起動になる() {
        assert!(!args(&["shiori-folio.exe"]).enabled);
        assert!(args(&["shiori-folio.exe", "--self-test"]).enabled);
        let m = args(&["x", "--self-test", "--self-test-break=library"]);
        assert_eq!(m.break_target.as_deref(), Some("library"));
        // 自己確認の起動でなければ、壊す指定は効かない
        assert_eq!(args(&["x", "--self-test-break=library"]).break_target, None);
    }
}

/// 自己確認(段階3): クリップボードの中身を読み戻す。ふつうの起動では何も返さない。
#[tauri::command]
pub fn self_test_clipboard_text() -> Option<String> {
    if !is_enabled() {
        return None;
    }
    use tauri_plugin_clipboard_manager::ClipboardExt;
    crate::notify::app()?.clipboard().read_text().ok()
}

/// 自己確認(段階3): Claudeと要確認の知らせを、試しに入口へ入れる(本当の終わりや増えを待たないため)。
#[tauri::command]
pub fn self_test_notify(kind: String) -> Option<u64> {
    if !is_enabled() {
        return None;
    }
    let kind = match kind.as_str() {
        "claude" => crate::notify::Kind::Claude,
        "pending" => crate::notify::Kind::Pending,
        _ => return None,
    };
    Some(crate::notify::show(kind, "自己確認の知らせ", "試しに出した知らせ", None))
}
