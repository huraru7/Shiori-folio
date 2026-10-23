//! 拡張機能から詩織への頼みごとの窓口(詩織Ver6.0の段階1)。
//!
//! 画面の部品は`postMessage`で、別のプログラムは標準入出力で頼み、ここに集まる。頼みごとの名前から
//! 要る「使うもの」を決め、説明書の`uses`に無ければ断る(承認済みであることは`Hub::ready`で確かめる)。
//! 断ったときはエラーコードを返し、ログに書く。

use super::manifest::Manifest;
use super::HubState;
use crate::error_codes::{self, ErrorCode};
use crate::shiori_log;
use serde::Serialize;
use serde_json::Value;

/// 頼みごとの名前と、要る使うもの(`None`は全員に貸すもの)、もう作ってあるか。
struct MethodDef {
    method: &'static str,
    uses: Option<&'static str>,
    ready: bool,
}

const METHODS: &[MethodDef] = &[
    MethodDef { method: "log.write", uses: None, ready: true },
    // 段階1のステップ6で作る
    MethodDef { method: "storage.get", uses: Some("storage"), ready: false },
    MethodDef { method: "storage.set", uses: Some("storage"), ready: false },
    MethodDef { method: "storage.delete", uses: Some("storage"), ready: false },
    MethodDef { method: "library.search", uses: Some("library.read"), ready: false },
    MethodDef { method: "shelf.search", uses: Some("library.read"), ready: false },
    MethodDef { method: "ext.call", uses: Some("ext.call"), ready: false },
    // 段階3で作る
    MethodDef { method: "clipboard.write", uses: Some("clipboard.write"), ready: false },
    MethodDef { method: "notify.show", uses: Some("notify"), ready: false },
    MethodDef { method: "schedule.add", uses: Some("schedule"), ready: false },
    MethodDef { method: "schedule.cancel", uses: Some("schedule"), ready: false },
    MethodDef { method: "llm.ask", uses: Some("llm.ask"), ready: false },
];

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct CallError {
    pub code: &'static str,
    pub message: String,
}

impl CallError {
    pub fn new(code: ErrorCode, detail: impl AsRef<str>) -> Self {
        let detail = detail.as_ref();
        let message = if detail.is_empty() { code.message.to_string() } else { format!("{}({detail})", code.message) };
        Self { code: code.code, message }
    }
}

/// 頼みごとを受けてよいかを決める。受けてよければ、その頼みごとの定め。
fn authorize(manifest: &Manifest, method: &str) -> Result<&'static MethodDef, CallError> {
    let def = METHODS
        .iter()
        .find(|d| d.method == method)
        .ok_or_else(|| CallError::new(error_codes::EXT_010, method))?;
    if let Some(uses) = def.uses {
        if !manifest.uses.iter().any(|u| u == uses) {
            return Err(CallError::new(error_codes::EXT_011, format!("{method}には{uses}が要る")));
        }
    }
    if !def.ready {
        return Err(CallError::new(error_codes::EXT_012, method));
    }
    Ok(def)
}

/// 頼みごとを1つ処理する。`name`は、詩織の側で確かめた拡張機能の名前(拡張機能が名乗った名前は使わない)。
pub fn call(state: &HubState, name: &str, method: &str, params: &Value) -> Result<Value, CallError> {
    let manifest = {
        let hub = state.0.lock().map_err(|e| CallError::new(error_codes::EXT_009, e.to_string()))?;
        hub.ready(name)
            .and_then(|e| e.manifest.clone())
            .ok_or_else(|| CallError::new(error_codes::EXT_009, name))?
    };
    let result = authorize(&manifest, method).and_then(|_| run(name, method, params));
    match &result {
        Ok(_) => shiori_log::debug("HUB", "頼みごとを受けた", &[("name", &name), ("method", &method)]),
        Err(e) => shiori_log::warn(e.code, "頼みごとを断った", &[("name", &name), ("method", &method), ("理由", &e.message)]),
    }
    result
}

fn run(name: &str, method: &str, params: &Value) -> Result<Value, CallError> {
    match method {
        "log.write" => log_write(name, params),
        _ => Err(CallError::new(error_codes::EXT_012, method)),
    }
}

/// 拡張機能のログ。部品名に拡張機能の名前を使う。`fields`は文字列の値だけを受ける。
fn log_write(name: &str, params: &Value) -> Result<Value, CallError> {
    let bad = |d: &str| CallError::new(error_codes::EXT_014, d);
    let event = params.get("event").and_then(Value::as_str).ok_or_else(|| bad("eventが要る"))?;
    let level = match params.get("level").and_then(Value::as_str).unwrap_or("info") {
        "error" => shiori_log::Level::Error,
        "warn" => shiori_log::Level::Warn,
        "info" => shiori_log::Level::Info,
        "debug" => shiori_log::Level::Debug,
        other => return Err(bad(&format!("levelはerror・warn・info・debugのどれか: {other}"))),
    };
    let fields: Vec<(String, String)> = match params.get("fields") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
            .collect(),
        Some(_) => return Err(bad("fieldsはキーと値の組")),
    };
    let pairs: Vec<(&str, &dyn std::fmt::Display)> =
        fields.iter().map(|(k, v)| (k.as_str(), v as &dyn std::fmt::Display)).collect();
    // エラーのときは、拡張機能のエラーコード(説明書のerrors)をpartに使える
    let part = params
        .get("code")
        .and_then(Value::as_str)
        .filter(|c| c.starts_with(&format!("{name}-")))
        .unwrap_or(name);
    shiori_log::write(level, part, event, &pairs);
    Ok(Value::Null)
}

/// 画面の部品からの頼みごと。本体の画面が、送り主のiframeから拡張機能の名前を決めて渡す。
#[tauri::command]
pub fn ext_call(
    state: tauri::State<'_, HubState>,
    name: String,
    method: String,
    params: Option<Value>,
) -> Result<Value, CallError> {
    call(&state, &name, &method, &params.unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::manifest::tests::{make_ext, ui_manifest, TempDir};
    use crate::hub::Hub;
    use std::sync::Mutex;

    fn state_with(uses: &str, approve: bool) -> (TempDir, HubState) {
        let t = TempDir::new("bridge");
        let exts = t.0.join("extensions");
        make_ext(&exts, "hello-ui", &ui_manifest("hello-ui", uses));
        let mut hub = Hub::load_from(exts, t.0.join("data"));
        if approve {
            hub.approve("hello-ui").unwrap();
        }
        (t, HubState(Mutex::new(hub)))
    }

    #[test]
    fn ログは全員に貸す() {
        let (_t, state) = state_with("", true);
        let params = serde_json::json!({ "event": "押された", "fields": { "回": 1 } });
        assert_eq!(call(&state, "hello-ui", "log.write", &params), Ok(Value::Null));
        let bad = serde_json::json!({ "level": "loud", "event": "x" });
        assert_eq!(call(&state, "hello-ui", "log.write", &bad).unwrap_err().code, "EXT-014");
    }

    #[test]
    fn 断る理由ごとのエラーコード() {
        let (_t, state) = state_with(r#""storage""#, true);
        let p = Value::Null;
        // 説明書に書いていない
        assert_eq!(call(&state, "hello-ui", "clipboard.write", &p).unwrap_err().code, "EXT-011");
        // 知らない頼みごと
        assert_eq!(call(&state, "hello-ui", "fs.read", &p).unwrap_err().code, "EXT-010");
        // 書いてあるが、まだ作っていない
        assert_eq!(call(&state, "hello-ui", "storage.get", &p).unwrap_err().code, "EXT-012");
        // 無い拡張機能
        assert_eq!(call(&state, "other", "log.write", &p).unwrap_err().code, "EXT-009");
    }

    #[test]
    fn 承認していない拡張機能は頼めない() {
        let (_t, state) = state_with("", false);
        let params = serde_json::json!({ "event": "x" });
        assert_eq!(call(&state, "hello-ui", "log.write", &params).unwrap_err().code, "EXT-009");
    }
}
