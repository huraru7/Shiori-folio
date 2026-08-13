//! コントロールパネルの「モデル切替」向け。
//!
//! 切替前のVRAM見積もりに、実際にモデルを起動して計測する方式は使わない
//! (それ自体が「2つのモデルが同時にVRAMへ乗る」という避けたい状況を作ってしまうため)。
//! 代わりに、事前に分かっている実測値をmodels/vram_estimates.jsonに保存しておき、
//! 未知のモデルはファイルサイズからの概算(×1.15)で仮置きする。切替が成功した
//! 実測値(切替前後のVRAM使用量の差分)を都度この実測値に書き戻すことで、
//! 使うほど精度が上がっていく設計にしている。

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub file_name: String,
    pub size_mb: u64,
    pub vram_estimate_gb: f64,
    pub is_measured: bool,
    pub is_current: bool,
}

fn estimates_path(root: &Path) -> PathBuf {
    root.join("models").join("vram_estimates.json")
}

fn load_estimates(root: &Path) -> HashMap<String, f64> {
    std::fs::read_to_string(estimates_path(root))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

// GGUF量子化モデルは経験的にファイルサイズの1.1〜1.2倍程度のVRAMを使う傾向がある
// (KVキャッシュ・activation分のオーバーヘッド)ため、未計測モデルの暫定値として採用する。
const SIZE_ESTIMATE_FACTOR: f64 = 1.15;

// (見積もりGB, 実測かどうか)を返す。実測値がmodels/vram_estimates.jsonにあればそれを、
// なければファイルサイズからの概算を返す。
pub fn estimate_for(root: &Path, file_name: &str, size_mb: u64) -> (f64, bool) {
    let estimates = load_estimates(root);
    if let Some(&gb) = estimates.get(file_name) {
        (gb, true)
    } else {
        let estimated_gb = (size_mb as f64 / 1024.0) * SIZE_ESTIMATE_FACTOR;
        (estimated_gb, false)
    }
}

pub fn record_measurement(root: &Path, file_name: &str, vram_gb: f64) -> Result<(), String> {
    let mut estimates = load_estimates(root);
    estimates.insert(file_name.to_string(), (vram_gb * 100.0).round() / 100.0);
    let text = serde_json::to_string_pretty(&estimates).map_err(|e| e.to_string())?;
    std::fs::write(estimates_path(root), text)
        .map_err(|e| format!("vram_estimates.jsonの書き込みに失敗: {e}"))
}

pub fn list_models(root: &Path, current_file_name: &str) -> Result<Vec<ModelInfo>, String> {
    let dir = root.join("models").join("llm");
    let mut out = Vec::new();
    for entry in
        std::fs::read_dir(&dir).map_err(|e| format!("models/llmの読み込みに失敗: {e}"))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("gguf") {
            continue;
        }
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let size_mb = entry.metadata().map_err(|e| e.to_string())?.len() / (1024 * 1024);
        let (vram_estimate_gb, is_measured) = estimate_for(root, &file_name, size_mb);
        out.push(ModelInfo {
            is_current: file_name == current_file_name,
            file_name,
            size_mb,
            vram_estimate_gb,
            is_measured,
        });
    }
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(out)
}
