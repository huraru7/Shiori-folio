//! 動作の記録。本体の `src-tauri/src/shiori_log.rs`(詩織Ver6.0の段階1)と同じ形で書く。
//!
//! このプログラムは本体と別のクレートなので、本体の部品を使えない。そこで、同じ決まりを
//! ここでも守る小さな写しを置く。
//! - 1行の形: `時刻(ミリ秒まで) 重さ [部品] 出来事 キー=値…`
//! - ファイル: `data/logs/rag-20261023-140512.log`(起動1回ごと)
//! - 消す基準: `rag-` のファイルを直近の `keep_runs` 個だけ残し、`rag-` の合計が `max_total_mb` を超えたら古いものから消す
//! - 記事の本文は書かない(検索語と、結果のタイトル・場所・点数まで)

use chrono::{DateTime, Local};
use serde::Deserialize;
use std::fmt::Display;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const PREFIX: &str = "rag-";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Error => "エラー",
            Level::Warn => "注意",
            Level::Info => "情報",
            Level::Debug => "詳細",
        }
    }
}

/// `config.json` の `log` の項目。本体の `LogSettings` と同じ名前・既定値。
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct LogSettings {
    pub detail: bool,
    pub keep_runs: usize,
    pub max_total_mb: u64,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self { detail: false, keep_runs: 30, max_total_mb: 100 }
    }
}

static SINK: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static DETAIL: AtomicBool = AtomicBool::new(false);

/// 起動時に1回呼ぶ。フォルダを作れないときも、標準エラーには出し続ける。
pub fn init(dir: &Path, settings: &LogSettings) {
    DETAIL.store(settings.detail, Ordering::Relaxed);
    let file = match fs::create_dir_all(dir) {
        Ok(()) => {
            let opened = open_file(dir, Local::now());
            prune(dir, settings, opened.as_ref().map(|(_, name)| name.as_str()));
            opened.map(|(f, _)| f)
        }
        Err(e) => {
            eprintln!("[LOG-001] ログのフォルダを作れない: {} ({e})", dir.display());
            None
        }
    };
    let _ = SINK.set(Mutex::new(file));
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write(Level::Error, "RAG", "panic", &[("内容", &info)]);
        default_hook(info);
    }));
}

/// 1行を書く。標準エラーにも写す(本体の起動待ちや開発中の端末で読めるように)。
pub fn write(level: Level, part: &str, event: &str, fields: &[(&str, &dyn Display)]) {
    if level == Level::Debug && !DETAIL.load(Ordering::Relaxed) {
        return;
    }
    let line = format_line(Local::now(), level, part, event, fields);
    eprintln!("{line}");
    if let Some(sink) = SINK.get()
        && let Ok(mut file) = sink.lock()
        && let Some(file) = file.as_mut()
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.write_all(b"\n");
    }
}

pub fn error(part: &str, event: &str, fields: &[(&str, &dyn Display)]) {
    write(Level::Error, part, event, fields);
}

pub fn warn(part: &str, event: &str, fields: &[(&str, &dyn Display)]) {
    write(Level::Warn, part, event, fields);
}

pub fn info(part: &str, event: &str, fields: &[(&str, &dyn Display)]) {
    write(Level::Info, part, event, fields);
}

pub fn debug(part: &str, event: &str, fields: &[(&str, &dyn Display)]) {
    write(Level::Debug, part, event, fields);
}

pub fn format_line(
    time: DateTime<Local>,
    level: Level,
    part: &str,
    event: &str,
    fields: &[(&str, &dyn Display)],
) -> String {
    let mut line = format!(
        "{} {} [{}] {}",
        time.format("%Y-%m-%d %H:%M:%S%.3f"),
        level.label(),
        part,
        one_line(event)
    );
    for (key, value) in fields {
        line.push(' ');
        line.push_str(key);
        line.push('=');
        line.push_str(&quote_value(&value.to_string()));
    }
    line
}

fn one_line(s: &str) -> String {
    s.replace('\r', "").replace('\n', "\\n")
}

fn quote_value(value: &str) -> String {
    let value = one_line(value);
    if value.is_empty() || value.contains([' ', '　', '=', '"']) {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value
    }
}

/// 起動1回ごとに新しく作る(同じ秒に2回起動したら `-2` などを付ける)。
fn open_file(dir: &Path, now: DateTime<Local>) -> Option<(File, String)> {
    let stem = format!("{PREFIX}{}", now.format("%Y%m%d-%H%M%S"));
    for n in 1..100 {
        let name = if n == 1 { format!("{stem}.log") } else { format!("{stem}-{n}.log") };
        match OpenOptions::new().write(true).create_new(true).open(dir.join(&name)) {
            Ok(f) => return Some((f, name)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                eprintln!("[LOG-002] ログのファイルを開けない: {} ({e})", dir.join(&name).display());
                return None;
            }
        }
    }
    None
}

fn is_rag_log(name: &str) -> bool {
    name.ends_with(".log")
        && name.strip_prefix(PREFIX).and_then(|r| r.as_bytes().first().copied()).is_some_and(|b| b.is_ascii_digit())
}

/// 消すファイルを選ぶ。`files` は(名前, 大きさ)。名前に時刻が入っているので、名前の順が古い順。
pub fn choose_removals(files: &[(String, u64)], settings: &LogSettings, keep: Option<&str>) -> Vec<String> {
    let mut ours: Vec<&(String, u64)> = files.iter().filter(|(n, _)| is_rag_log(n)).collect();
    ours.sort_by(|a, b| b.0.cmp(&a.0));
    let mut remove = Vec::new();
    let mut total = 0u64;
    let limit = settings.max_total_mb.saturating_mul(1024 * 1024);
    for (i, (name, size)) in ours.iter().enumerate() {
        if Some(name.as_str()) == keep {
            total += size;
            continue;
        }
        if i >= settings.keep_runs.max(1) || total + size > limit {
            remove.push(name.clone());
        } else {
            total += size;
        }
    }
    remove
}

fn prune(dir: &Path, settings: &LogSettings, keep: Option<&str>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let files: Vec<(String, u64)> = entries
        .flatten()
        .filter_map(|e| Some((e.file_name().to_string_lossy().into_owned(), e.metadata().ok()?.len())))
        .collect();
    for name in choose_removals(&files, settings, keep) {
        let _ = fs::remove_file(dir.join(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn 一行の形が本体と同じ() {
        let t = Local.with_ymd_and_hms(2026, 10, 23, 14, 5, 12).unwrap();
        let line = format_line(t, Level::Info, "RAG", "検索した", &[("語", &"起動 遅い"), ("件数", &3)]);
        assert_eq!(line, "2026-10-23 14:05:12.000 情報 [RAG] 検索した 語=\"起動 遅い\" 件数=3");
    }

    #[test]
    fn 古いものから消す() {
        let files: Vec<(String, u64)> = (1..=5)
            .map(|i| (format!("rag-2026102{i}-000000.log"), 10))
            .chain([("shiori-20261020-000000.log".to_string(), 10)])
            .collect();
        let s = LogSettings { detail: false, keep_runs: 3, max_total_mb: 100 };
        let mut removed = choose_removals(&files, &s, Some("rag-20261025-000000.log"));
        removed.sort();
        assert_eq!(removed, vec!["rag-20261021-000000.log", "rag-20261022-000000.log"]);
    }
}
