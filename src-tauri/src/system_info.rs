//! コントロールパネルのシステムモニター向け、読み取り専用の状態取得。
//!
//! GPU情報(使用量・温度・プロセス別VRAM)はsysinfoでは取得できないため
//! `nvidia-smi`をサブプロセスで呼び出す。nvidia-smiが無い環境(GPU非搭載機、
//! PATHが通っていない等)では単にNoneを返し、フロントエンド側は
//! 「GPU情報を取得できません」と表示する想定。

use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vram_used_mb: u64,
    pub vram_total_mb: u64,
    pub temperature_c: Option<u32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub running: bool,
    pub vram_mb: Option<u64>,
    pub ram_mb: Option<u64>,
    // llama-server(LLM)のみ、現在ロードしているモデル名を添える。
    pub model_name: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiskThroughput {
    pub read_mb_per_sec: f64,
    pub write_mb_per_sec: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub drive: String,
    // "SSD" / "HDD" / "不明"
    pub kind: String,
    pub file_system: String,
    pub total_gb: f64,
    pub free_gb: f64,
    pub is_removable: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PowerInfo {
    pub on_battery: bool,
    pub battery_percent: Option<f64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CpuDetail {
    pub model: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub frequency_mhz: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfoDto {
    pub gpu: Option<GpuInfo>,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub cpu_usage_percent: f32,
    pub os: String,
    pub cpu: CpuDetail,
    pub services: Vec<ServiceInfo>,
    pub disk_throughput: Option<DiskThroughput>,
    pub storage: Option<StorageInfo>,
    pub power: Option<PowerInfo>,
}

// `nvidia-smi --query-gpu=... --format=csv,noheader,nounits`の出力例:
// "NVIDIA GeForce RTX 4060 Laptop GPU, 2954, 8188, 62"
pub fn query_gpu_info() -> Option<GpuInfo> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?;
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if parts.len() < 3 {
        return None;
    }

    Some(GpuInfo {
        name: parts[0].to_string(),
        vram_used_mb: parts[1].parse().ok()?,
        vram_total_mb: parts[2].parse().ok()?,
        temperature_c: parts.get(3).and_then(|s| s.parse().ok()),
    })
}

// `nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits`
// でGPUを使用中の各プロセスのVRAM使用量(MB)を取得する。GPUを使っていない、または
// nvidia-smi自体が使えない環境では空のマップを返す(呼び出し元はNone/0扱いにする)。
pub fn query_process_vram_map() -> HashMap<u32, u64> {
    let mut map = HashMap::new();
    let Ok(output) = Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=pid,used_memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
    else {
        return map;
    };
    if !output.status.success() {
        return map;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 2 {
            continue;
        }
        if let (Ok(pid), Ok(mb)) = (parts[0].parse::<u32>(), parts[1].parse::<u64>()) {
            map.insert(pid, mb);
        }
    }
    map
}

#[cfg(windows)]
pub fn query_power_info() -> Option<PowerInfo> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    unsafe {
        let mut status: SYSTEM_POWER_STATUS = std::mem::zeroed();
        if GetSystemPowerStatus(&mut status) == 0 {
            return None;
        }
        // ACLineStatus: 0=バッテリー駆動, 1=AC接続, 255=不明
        let on_battery = status.ACLineStatus == 0;
        // BatteryLifePercent: 0-100、255=不明(バッテリー非搭載機など)
        let battery_percent = if status.BatteryLifePercent <= 100 {
            Some(status.BatteryLifePercent as f64)
        } else {
            None
        };
        Some(PowerInfo {
            on_battery,
            battery_percent,
        })
    }
}

#[cfg(not(windows))]
pub fn query_power_info() -> Option<PowerInfo> {
    None
}

/// `drive_letter`(例: 'C')の累積(読み込みバイト数, 書き込みバイト数)を返す。
/// Windows以外、または取得に失敗した場合はNone。
#[cfg(windows)]
pub fn query_disk_counters(drive_letter: char) -> Option<(i64, i64)> {
    crate::disk_io::query_disk_counters(drive_letter)
}

#[cfg(not(windows))]
pub fn query_disk_counters(_drive_letter: char) -> Option<(i64, i64)> {
    None
}
