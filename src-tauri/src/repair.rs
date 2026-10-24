//! 起動時の自動修復(詩織Ver6.0の段階6)。
//!
//! 詩織が作ったもの(作り直せるもの)だけを直し、直したことを部品名 `FIX` の行でログに書く
//! (診断 `diagnose.rs` がこの行を「直した」として拾う)。自己確認の「自動修復」が試しのルートで直した
//! 分は、部品名 `FIX-SELFTEST` で書き、診断には出さない。ふらるが書いたもの(library・棚・memory)と
//! 会話の記録(`shiori.db`)は直さない(診断で報告するだけ)。
//!
//! ほかの場所にある自動修復: 列の並び(`UI-001`、sidebar_layout.rs)、承認の記録(`EXT-007`、hub)、
//! 別のプログラムの再起動(`EXT-015`)、索引の1記事(`RAG-003`、shiori-rag)。
//!
//! 企画書は plan-shiori-ver6-s6-doctor-20261024(詩織のlibrary)。

use crate::error_codes::{self, ErrorCode};
use crate::{shared_daemon, shiori_log};
use chrono::Local;
use std::fmt::Display;
use std::path::{Path, PathBuf};

pub use crate::diagnose::{REPAIR_PART, REPAIR_REHEARSAL_PART};

thread_local! {
    /// このスレッドで `self_check` が走っているあいだだけ true(試しの修復を本当の修復と分けて書く)。
    static REHEARSAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `self_check` のあいだ `REHEARSAL` を立て、抜けるときに(途中で戻っても)下ろす。
struct Rehearsal;
impl Rehearsal {
    fn start() -> Self {
        REHEARSAL.with(|r| r.set(true));
        Self
    }
}
impl Drop for Rehearsal {
    fn drop(&mut self) {
        REHEARSAL.with(|r| r.set(false));
    }
}

fn repair_part() -> &'static str {
    if REHEARSAL.with(|r| r.get()) { REPAIR_REHEARSAL_PART } else { REPAIR_PART }
}

/// 壊れた `config.json` を、写しも無いときに戻す既定の設定(リポジトリの `config.json`)。
const DEFAULT_CONFIG: &str = include_str!("../../config.json");

/// 前回正常に起動したときの `config.json` の写し。
pub fn last_good_config_path(root: &Path) -> PathBuf {
    root.join("data").join("config.last-good.json")
}

/// ログを始める前に見ておくもの(ログを始めると `data/logs` が作られてしまうため)。
pub struct Before {
    logs_missing: bool,
}

pub fn before_log(root: &Path) -> Before {
    Before { logs_missing: !root.join("data").join("logs").is_dir() }
}

/// 直したことを1件1行で書く。
pub fn fixed(code: ErrorCode, fields: &[(&str, &dyn Display)]) {
    let mut all: Vec<(&str, &dyn Display)> = vec![("コード", &code.code)];
    all.extend_from_slice(fields);
    shiori_log::warn(repair_part(), code.message, &all);
}

/// 起動の最初(ログを始めた直後、設定を読む前)に呼ぶ。
pub fn run_at_startup(root: &Path, before: Before) {
    work_dirs(root, before);
    stale_locks(root);
    config(root);
    index(root);
}

fn work_dirs(root: &Path, before: Before) {
    if before.logs_missing {
        fixed(error_codes::APP_001, &[("フォルダ", &"data/logs")]);
    }
    for dir in ["extensions", "ui"] {
        let path = root.join("data").join(dir);
        if path.is_dir() {
            continue;
        }
        match std::fs::create_dir_all(&path) {
            Ok(()) => fixed(error_codes::APP_001, &[("フォルダ", &format!("data/{dir}"))]),
            Err(e) => shiori_log::error(error_codes::APP_004.code, error_codes::APP_004.message, &[("場所", &path.display()), ("理由", &e)]),
        }
    }
}

/// 記録のPIDが動いていないロックを消す。残しておくと、ほかのプロセスに同じPIDが振られたとき、
/// 止める処理(`kill_daemon`)が関係の無いプロセスを止めてしまう恐れがある。
fn stale_locks(root: &Path) {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for name in [".shiori-embed.lock", ".shiori-rag.lock"] {
        let Some(pid) = shared_daemon::read_daemon_pid(root, name) else { continue };
        if shared_daemon::daemon_pid_if_alive(&sys, root, name).is_some() {
            continue;
        }
        let path = root.join("data").join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => fixed(error_codes::APP_002, &[("ロック", &name), ("PID", &pid)]),
            Err(e) => shiori_log::error(error_codes::APP_004.code, error_codes::APP_004.message, &[("場所", &path.display()), ("理由", &e)]),
        }
    }
}

/// `config.json` が読めなければ、脇へ退けてから、前回の写し(無ければ既定の設定)に戻す。
fn config(root: &Path) {
    let Err(reason) = crate::load_config(root) else { return };
    let path = root.join("config.json");
    let last_good = std::fs::read_to_string(last_good_config_path(root))
        .ok()
        .filter(|t| serde_json::from_str::<crate::AppConfig>(t).is_ok());
    let (code, text) = match last_good {
        Some(t) => (error_codes::CFG_001, t),
        None => (error_codes::CFG_002, DEFAULT_CONFIG.to_string()),
    };
    let aside = root.join(format!("config.json.broken-{}", Local::now().format("%Y%m%d-%H%M%S")));
    let moved = if path.exists() { std::fs::rename(&path, &aside).map(|()| Some(aside)) } else { Ok(None) };
    match moved.and_then(|aside| std::fs::write(&path, text).map(|()| aside)) {
        Ok(aside) => fixed(
            code,
            &[("理由", &reason), ("退けた先", &aside.map(|p| p.display().to_string()).unwrap_or_else(|| "(無かった)".into()))],
        ),
        Err(e) => shiori_log::error(error_codes::CFG_003.code, error_codes::CFG_003.message, &[("理由", &reason), ("直せない理由", &e)]),
    }
}

/// 索引のフォルダが無ければ、そう書く(作り直すのは、RAGの起動の同期)。
fn index(root: &Path) {
    let dir = crate::rag_vectordb_dir(root);
    if crate::rust_rag_exe().is_some() && !dir.is_dir() {
        fixed(error_codes::APP_003, &[("フォルダ", &dir.display())]);
    }
}

/// 起動して画面が出たあとに呼ぶ。いまの `config.json` が読めれば、前回正常に起動したときの写しにする。
/// 設定を変えるたびに写さないのは、壊れた設定を変えた直後に写してしまわないため(企画書で決めた)。
pub fn save_last_good_config(root: &Path) {
    if crate::load_config(root).is_err() {
        return;
    }
    let to = last_good_config_path(root);
    if let Err(e) = std::fs::copy(root.join("config.json"), &to) {
        shiori_log::error(error_codes::CFG_004.code, error_codes::CFG_004.message, &[("場所", &to.display()), ("理由", &e)]);
    }
}

/// 自己確認の起動(段階6)で、起動時の修復が本当に直すかを確かめる。本物の設定や記録には触れず、
/// SSDの `data/_selftest-repair/` に試しのためだけのルートを作り、壊したものを用意して、起動時と同じ
/// 修復を当てる(exfatのSSDの上で確かめるため、PCの一時フォルダではなくSSDに作る)。終わったら消す。
/// 戻り値は、全部直ったかと、ログの欄。
pub fn self_check(root: &Path) -> (bool, Vec<(&'static str, String)>) {
    let _rehearsal = Rehearsal::start();
    let f = root.join("data").join("_selftest-repair");
    let _ = std::fs::remove_dir_all(&f);
    let data = f.join("data");
    let prepared = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(data.join("extensions"))?;
        std::fs::copy(root.join("config.json"), last_good_config_path(&f))?;
        std::fs::write(f.join("config.json"), "{ 壊れている")?;
        // 動いていないPIDとして、ありえない大きな値を使う
        std::fs::write(data.join(".shiori-rag.lock"), "4294967")?;
        std::fs::write(data.join("extensions").join("_approvals.json"), "壊れている")
    })();
    if let Err(e) = prepared {
        let _ = std::fs::remove_dir_all(&f);
        return (false, vec![("用意できない", e.to_string())]);
    }

    run_at_startup(&f, before_log(&f));
    let _ = crate::hub::Hub::load(&f);
    let sidebar = data.join("ui").join("sidebar.json");
    let sidebar_ready = std::fs::write(&sidebar, "壊れている").is_ok();
    let _ = crate::sidebar_layout::load(&sidebar);

    let names = |dir: &Path| -> Vec<String> {
        std::fs::read_dir(dir).map(|e| e.flatten().map(|x| x.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
    };
    let aside = |dir: &Path, prefix: &str| names(dir).iter().any(|n| n.starts_with(prefix));
    let checks = [
        ("設定を前回の写しに戻した", std::fs::read(f.join("config.json")).ok() == std::fs::read(last_good_config_path(&f)).ok()),
        ("壊れた設定を残した", aside(&f, "config.json.broken-")),
        ("古いロックを消した", !data.join(".shiori-rag.lock").exists()),
        ("作業用フォルダを作った", data.join("ui").is_dir()),
        ("承認の記録を退けた", aside(&data.join("extensions"), "_approvals.json.broken-")),
        ("列の並びの記録を退けた", sidebar_ready && aside(&data.join("ui"), "sidebar.json.broken-")),
    ];
    let cleaned = std::fs::remove_dir_all(&f).is_ok();
    let pass = cleaned && checks.iter().all(|(_, ok)| *ok);
    let mut fields: Vec<(&'static str, String)> = checks.iter().map(|(k, ok)| (*k, ok.to_string())).collect();
    fields.push(("片付けた", cleaned.to_string()));
    (pass, fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRoot(PathBuf);
    impl TempRoot {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("shiori-repair-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("data")).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn 既定の設定は読める() {
        assert!(serde_json::from_str::<crate::AppConfig>(DEFAULT_CONFIG).is_ok());
    }

    #[test]
    fn 壊れた設定は脇へ退けて前回の写しに戻す() {
        let t = TempRoot::new("cfg-last-good");
        let good = DEFAULT_CONFIG.replace("\"hotkey\"", "\"_note\": \"写し\", \"hotkey\"");
        std::fs::write(last_good_config_path(&t.0), &good).unwrap();
        std::fs::write(t.0.join("config.json"), "{ 壊れている").unwrap();
        config(&t.0);
        assert_eq!(std::fs::read_to_string(t.0.join("config.json")).unwrap(), good);
        let aside: Vec<_> = std::fs::read_dir(&t.0)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("config.json.broken-"))
            .collect();
        assert_eq!(aside.len(), 1, "壊れた設定は消さずに残す");
    }

    #[test]
    fn 写しが無ければ既定の設定に戻す() {
        let t = TempRoot::new("cfg-default");
        std::fs::write(t.0.join("config.json"), "壊れている").unwrap();
        config(&t.0);
        assert_eq!(std::fs::read_to_string(t.0.join("config.json")).unwrap(), DEFAULT_CONFIG);
    }

    #[test]
    fn 読める設定には触らない() {
        let t = TempRoot::new("cfg-ok");
        std::fs::write(t.0.join("config.json"), DEFAULT_CONFIG).unwrap();
        config(&t.0);
        assert_eq!(std::fs::read_dir(&t.0).unwrap().count(), 2, "config.jsonとdataだけ");
    }

    #[test]
    fn 動いていないPIDのロックを消し_ほかは残す() {
        let t = TempRoot::new("locks");
        // 動いていないPIDとして、ありえない大きな値を使う
        std::fs::write(t.0.join("data").join(".shiori-rag.lock"), "4294967").unwrap();
        std::fs::write(t.0.join("data").join(".shiori-embed.lock"), std::process::id().to_string()).unwrap();
        stale_locks(&t.0);
        assert!(!t.0.join("data").join(".shiori-rag.lock").exists());
        assert!(t.0.join("data").join(".shiori-embed.lock").exists(), "動いているPIDのロックは残す");
    }

    #[test]
    fn 自己確認の修復の確かめは全部直して片付ける() {
        let t = TempRoot::new("self-check");
        std::fs::write(t.0.join("config.json"), DEFAULT_CONFIG).unwrap();
        let (pass, fields) = self_check(&t.0);
        assert!(pass, "{fields:?}");
        assert!(!t.0.join("data").join("_selftest-repair").exists());
        assert_eq!(std::fs::read_to_string(t.0.join("config.json")).unwrap(), DEFAULT_CONFIG, "本物の設定は触らない");
        assert_eq!(repair_part(), REPAIR_PART, "確かめが終わったら本当の修復の部品名に戻す");
    }

    #[test]
    fn 試しの修復は診断の直したに入らない部品名で書く() {
        {
            let _r = Rehearsal::start();
            assert_eq!(repair_part(), REPAIR_REHEARSAL_PART);
            // 診断は `[FIX]` を含む行だけを拾うので、試しの行は拾われない
            assert!(!format!("[{REPAIR_REHEARSAL_PART}]").contains(&format!("[{REPAIR_PART}]")));
        }
        assert_eq!(repair_part(), REPAIR_PART);
        // ほかのスレッドは、試しのあいだでも本当の修復の部品名のまま
        let _r = Rehearsal::start();
        assert_eq!(std::thread::spawn(repair_part).join().unwrap(), REPAIR_PART);
    }

    #[test]
    fn 無い作業用フォルダを作る() {
        let t = TempRoot::new("dirs");
        work_dirs(&t.0, Before { logs_missing: false });
        assert!(t.0.join("data").join("extensions").is_dir());
        assert!(t.0.join("data").join("ui").is_dir());
    }
}
