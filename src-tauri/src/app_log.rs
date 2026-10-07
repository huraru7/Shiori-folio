//! エラーの記録(詩織Ver4.0)。
//!
//! それまではエラーを`eprintln!`で標準エラーに出すだけで、詩織をダブルクリックで
//! 起動したときは何も残らず、使い始めてから起きた不具合の原因を追えなかった。
//! `data/logs/shiori-YYYYMMDD.log`へ日付ごとに追記し、古いものは起動時に消す。
//! プロンプトや記事の本文を丸ごとは残さないよう、1件の長さに上限を設ける。

use chrono::{Local, NaiveDate};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// ログを残す日数。これより古い日付のファイルは起動時に消す。
const KEEP_DAYS: i64 = 14;
/// 1件に残す最大の文字数。
const MAX_MESSAGE_CHARS: usize = 2000;
const FILE_PREFIX: &str = "shiori-";
const FILE_SUFFIX: &str = ".log";

static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
// 複数のスレッドから同時に書いても、行が混ざらないようにする。
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// 起動時に1回呼ぶ。置き場所を決め、古いログを消し、panicも記録するようにする。
pub fn init(dir: PathBuf) {
    if fs::create_dir_all(&dir).is_ok() {
        remove_expired(&dir, Local::now().date_naive());
    }
    let _ = LOG_DIR.set(dir);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write("PANIC", &info.to_string());
        default_hook(info);
    }));
}

/// 標準エラーに出し、ログのファイルにも追記する。ファイルに書けなくても、
/// 呼び出し元の処理は止めない(記録のために本来の処理を失敗させない)。
pub fn write(level: &str, message: &str) {
    eprintln!("[{level}] {message}");
    let Some(dir) = LOG_DIR.get() else {
        return;
    };
    let now = Local::now();
    let path = dir.join(file_name(now.date_naive()));
    let line = format!("{} [{level}] {}\n", now.format("%Y-%m-%d %H:%M:%S"), truncate(message));
    let _guard = WRITE_LOCK.lock();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

fn file_name(date: NaiveDate) -> String {
    format!("{FILE_PREFIX}{}{FILE_SUFFIX}", date.format("%Y%m%d"))
}

fn truncate(message: &str) -> String {
    if message.chars().count() <= MAX_MESSAGE_CHARS {
        return message.to_string();
    }
    let head: String = message.chars().take(MAX_MESSAGE_CHARS).collect();
    format!("{head}…(以下省略)")
}

/// `shiori-YYYYMMDD.log`のうち、`today`から`KEEP_DAYS`日より前のものを消す。
/// 名前の形が違うファイルには触らない。
fn remove_expired(dir: &Path, today: NaiveDate) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_expired(&name, today) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn is_expired(name: &str, today: NaiveDate) -> bool {
    let Some(date) = name
        .strip_prefix(FILE_PREFIX)
        .and_then(|rest| rest.strip_suffix(FILE_SUFFIX))
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y%m%d").ok())
    else {
        return false;
    };
    (today - date).num_days() > KEEP_DAYS
}

/// `format!`と同じ書き方で、エラーとして記録する。
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::app_log::write("ERROR", &format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn 古いログだけを消す対象にする() {
        let today = day("2026-10-20");
        assert!(is_expired("shiori-20261005.log", today));
        assert!(!is_expired("shiori-20261006.log", today));
        assert!(!is_expired("shiori-20261020.log", today));
        assert!(!is_expired("other-20200101.log", today));
        assert!(!is_expired("shiori-abc.log", today));
        assert!(!is_expired("._shiori-20200101.log", today));
    }

    #[test]
    fn 長いメッセージは切り詰める() {
        let long = "あ".repeat(MAX_MESSAGE_CHARS + 10);
        let cut = truncate(&long);
        assert!(cut.ends_with("…(以下省略)"));
        assert_eq!(cut.chars().count(), MAX_MESSAGE_CHARS + "…(以下省略)".chars().count());
        assert_eq!(truncate("短い"), "短い");
    }

    #[test]
    fn 日付ごとのファイル名() {
        assert_eq!(file_name(day("2026-10-07")), "shiori-20261007.log");
    }
}
