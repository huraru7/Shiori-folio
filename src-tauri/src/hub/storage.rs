//! 拡張機能ごとの置き場(詩織Ver6.0の段階1)。設定と状態を、キーと値で保存する。
//!
//! 置き場は `portable/data/extensions/{名前}/storage.json` の1つのファイル。キーはJSONの中の名前で、
//! ファイルの場所には使わないので、ほかの拡張機能の置き場には届かない(`{名前}`は詩織が確かめた名前)。

use super::bridge::CallError;
use crate::error_codes;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const STORAGE_FILE: &str = "storage.json";
/// キーの長さの上限(文字数)
const MAX_KEY_CHARS: usize = 200;
/// 置き場全体の大きさの上限(1MB)。大きなデータの置き場ではなく、設定と状態のためのもの
const MAX_BYTES: usize = 1024 * 1024;

fn file_of(dir: &Path) -> PathBuf {
    dir.join(STORAGE_FILE)
}

fn load(dir: &Path) -> Result<Map<String, Value>, CallError> {
    match std::fs::read_to_string(file_of(dir)) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Ok(map),
            _ => Err(CallError::new(error_codes::EXT_022, "置き場のファイルが壊れている")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(CallError::new(error_codes::EXT_022, e.to_string())),
    }
}

fn save(dir: &Path, map: &Map<String, Value>) -> Result<(), CallError> {
    let text = serde_json::to_string_pretty(map).map_err(|e| CallError::new(error_codes::EXT_022, e.to_string()))?;
    if text.len() > MAX_BYTES {
        return Err(CallError::new(error_codes::EXT_022, format!("置き場が{}KBを超える", MAX_BYTES / 1024)));
    }
    std::fs::create_dir_all(dir).map_err(|e| CallError::new(error_codes::EXT_022, e.to_string()))?;
    // 途中で落ちても壊れないよう、別のファイルに書いてから置き換える
    let tmp = dir.join(format!("{STORAGE_FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| CallError::new(error_codes::EXT_022, e.to_string()))?;
    std::fs::rename(&tmp, file_of(dir)).map_err(|e| CallError::new(error_codes::EXT_022, e.to_string()))
}

fn key_of(params: &Value) -> Result<&str, CallError> {
    let key = params
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| CallError::new(error_codes::EXT_014, "keyが要る(文字列)"))?;
    if key.is_empty() || key.chars().count() > MAX_KEY_CHARS {
        return Err(CallError::new(error_codes::EXT_014, format!("keyは1〜{MAX_KEY_CHARS}文字")));
    }
    Ok(key)
}

/// `storage.get {key}` → 値(無ければnull)
pub fn get(dir: &Path, params: &Value) -> Result<Value, CallError> {
    let key = key_of(params)?;
    Ok(load(dir)?.get(key).cloned().unwrap_or(Value::Null))
}

/// `storage.set {key, value}`
pub fn set(dir: &Path, params: &Value) -> Result<Value, CallError> {
    let key = key_of(params)?.to_string();
    let value = params.get("value").cloned().unwrap_or(Value::Null);
    let mut map = load(dir)?;
    map.insert(key, value);
    save(dir, &map)?;
    Ok(Value::Null)
}

/// `storage.delete {key}` → 消したか
pub fn delete(dir: &Path, params: &Value) -> Result<Value, CallError> {
    let key = key_of(params)?;
    let mut map = load(dir)?;
    let removed = map.remove(key).is_some();
    if removed {
        save(dir, &map)?;
    }
    Ok(Value::Bool(removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::manifest::tests::TempDir;
    use serde_json::json;

    #[test]
    fn 書いて読んで消せる() {
        let t = TempDir::new("storage");
        let dir = t.0.join("hello-ui");
        assert_eq!(get(&dir, &json!({ "key": "a" })).unwrap(), Value::Null);
        set(&dir, &json!({ "key": "a", "value": { "n": 1 } })).unwrap();
        assert_eq!(get(&dir, &json!({ "key": "a" })).unwrap(), json!({ "n": 1 }));
        assert_eq!(delete(&dir, &json!({ "key": "a" })).unwrap(), Value::Bool(true));
        assert_eq!(delete(&dir, &json!({ "key": "a" })).unwrap(), Value::Bool(false));
    }

    #[test]
    fn キーは置き場の外を指さない() {
        let t = TempDir::new("storage-key");
        let dir = t.0.join("hello-ui");
        set(&dir, &json!({ "key": "../hello-program/storage", "value": 1 })).unwrap();
        // 自分の置き場のファイルの中の1つのキーになるだけで、ほかのフォルダはできない
        assert!(!t.0.join("hello-program").exists());
        assert_eq!(get(&dir, &json!({ "key": "../hello-program/storage" })).unwrap(), json!(1));
    }

    #[test]
    fn 正しくない頼みと大きすぎる値は断る() {
        let t = TempDir::new("storage-bad");
        let dir = t.0.join("hello-ui");
        assert_eq!(get(&dir, &json!({})).unwrap_err().code, "EXT-014");
        assert_eq!(get(&dir, &json!({ "key": "" })).unwrap_err().code, "EXT-014");
        let big = "x".repeat(MAX_BYTES + 1);
        assert_eq!(set(&dir, &json!({ "key": "big", "value": big })).unwrap_err().code, "EXT-022");
    }
}
