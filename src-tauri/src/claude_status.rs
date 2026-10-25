//! Claudeモニター(詩織Ver3.7)向け、Claude Codeのセッション状況の読み取り。
//!
//! Claude Code側のフックが`data/claude-status/<session_id>.json`へ書いた状況を
//! 読むだけ(書き込み・掃除はClaude側の`portable/scripts/claude-report.js`が担う)。
//! 詩織が止まっている間もClaude側は書き続けるため、ここは起動時点の最新を返す。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// プロセスの生死を確かめられないセッション(別のPCのもの、pidが取れなかったもの)で、
/// 更新が止まったままの作業中/入力待ちを「応答なし」とみなすまでの時間。
/// フックは生存確認の通信をしないため、長いビルドや長い推論の間は無音になる。
/// 時間では死活を判定できないので、ここは長めの目安にとどめる。
const STALE_AFTER_MS: u64 = 60 * 60 * 1000;
/// 終了したセッションを一覧に残す時間。
const ENDED_VISIBLE_MS: u64 = 5 * 60 * 1000;
/// 生死を確かめられないセッションを、状態にかかわらず一覧から外す時間。
const HIDE_AFTER_MS: u64 = 12 * 60 * 60 * 1000;

/// Claude側のスクリプトが書くJSON(キーはスネークケース)。
#[derive(Deserialize)]
struct StatusFile {
    session_id: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    host: String,
    /// フックを起動したClaude Codeのプロセスid。取れなかったときは無い。
    #[serde(default)]
    pid: Option<u32>,
    state: String,
    started_at: u64,
    updated_at: u64,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    pub session_id: String,
    pub project: String,
    pub title: String,
    pub host: String,
    /// 表示用の状態。`working`/`waiting`/`idle`/`ended`/`stale`(応答なし)。
    pub state: String,
    pub started_at: u64,
    pub updated_at: u64,
}

/// Claude Codeのプロセスpidが、いま生きているかを調べる。pidの使い回しで別の
/// プロセスを生きていると誤認しないよう、プロセス名がclaudeであることも確かめる。
pub fn is_claude_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pid = Pid::from_u32(pid);
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::new(),
    );
    sys.process(pid).is_some_and(|p| {
        p.name()
            .to_string_lossy()
            .to_lowercase()
            .trim_end_matches(".exe")
            == "claude"
    })
}

/// `dir`配下の`*.json`を読み、表示対象のセッションを「作業中・入力待ち→待機→
/// それ以外」、同じ状態の中では更新が新しい順に並べて返す。
/// ディレクトリが無い(Claude側が一度も書いていない)場合は空を返す。
/// 壊れたファイルや書き込み途中の`.tmp`は無視する。
///
/// 死活の判定: このPC(`local_host`)のセッションでpidがあれば、プロセスの生死で決める
/// (`alive`)。生きていれば、無音がどれだけ続いても状態を信じる。死んでいて終了の報告
/// が無いものは、強制終了として「終了」扱いにする。別のPCのセッションやpidが無い
/// ものは確かめようがないので、更新が止まった時間による目安にフォールバックする。
pub fn read_sessions(
    dir: &Path,
    now_ms: u64,
    local_host: &str,
    alive: &dyn Fn(u32) -> bool,
) -> Vec<ClaudeSession> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut sessions: Vec<ClaudeSession> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".json") && !name.starts_with("._")
        })
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|text| serde_json::from_str::<StatusFile>(&text).ok())
        .filter_map(|f| to_session(f, now_ms, local_host, alive))
        .collect();
    sessions.sort_by(|a, b| {
        state_rank(&a.state)
            .cmp(&state_rank(&b.state))
            .then(b.updated_at.cmp(&a.updated_at))
    });
    sessions
}

fn to_session(
    f: StatusFile,
    now_ms: u64,
    local_host: &str,
    alive: &dyn Fn(u32) -> bool,
) -> Option<ClaudeSession> {
    let age = now_ms.saturating_sub(f.updated_at);
    // Some(true/false)=このPCで生死を確かめられた。None=確かめられない。
    let process_alive = match f.pid {
        Some(pid) if f.host == local_host => Some(alive(pid)),
        _ => None,
    };
    if process_alive.is_none() && age > HIDE_AFTER_MS {
        return None;
    }
    let state = match (f.state.as_str(), process_alive) {
        // 終了の報告が来ないままプロセスが消えた(強制終了など)。
        (s, Some(false)) if s != "ended" => "ended",
        ("ended", _) => "ended",
        ("working", Some(true)) => "working",
        ("waiting", Some(true)) => "waiting",
        ("working" | "waiting", None) if age > STALE_AFTER_MS => "stale",
        ("working", None) => "working",
        ("waiting", None) => "waiting",
        _ => "idle",
    };
    if state == "ended" && age > ENDED_VISIBLE_MS {
        return None;
    }
    Some(ClaudeSession {
        session_id: f.session_id,
        project: f.project,
        title: f.title,
        host: f.host,
        state: state.to_string(),
        started_at: f.started_at,
        updated_at: f.updated_at,
    })
}

/// 知らせを押したときに前に出すセッションのpid(詩織Ver6.0)。このPCのセッションで、
/// pidが分かり、まだ生きているときだけ返す。それ以外は、呼び出し側が「Claude」の画面へ回す。
pub fn local_pid(
    dir: &Path,
    session_id: &str,
    local_host: &str,
    alive: &dyn Fn(u32) -> bool,
) -> Option<u32> {
    // session_idはファイル名に使うので、パスとして働く文字を含むものは受けない
    if session_id.is_empty() || session_id.contains(['/', '\\', '.']) {
        return None;
    }
    let text = fs::read_to_string(dir.join(format!("{session_id}.json"))).ok()?;
    let f: StatusFile = serde_json::from_str(&text).ok()?;
    let pid = f.pid.filter(|_| f.host == local_host)?;
    alive(pid).then_some(pid)
}

/// pidのプロセスから親へたどった、pidと実行ファイルの並び(自分が先頭、根に近いものが後ろ)。
fn ancestors(pid: u32) -> Vec<(u32, std::path::PathBuf)> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_exe(UpdateKind::OnlyIfNotSet),
    );
    let mut chain = Vec::new();
    let mut current = Some(Pid::from_u32(pid));
    // 親の輪(起きないはずだが)で回り続けないよう、深さに上限を置く
    while let Some(p) = current.and_then(|p| sys.process(p)) {
        if chain.len() >= 64 {
            break;
        }
        chain.push((p.pid().as_u32(), p.exe().map(Path::to_path_buf).unwrap_or_default()));
        current = p.parent();
    }
    chain
}

/// Macで前に出すアプリ。根に近いほうから見て、最初に `.app/Contents/MacOS/` の中で動いている
/// プロセスのアプリを選ぶ。Claudeのデスクトップアプリの中の `claude` は、それ自体も `claude.app` の
/// 中にあるが、根に近いClaude.appが先に見つかる。VS Codeの端末も、Code.appの本体が先に見つかる。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mac_app_bundle(exes: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
    exes.iter().rev().find_map(|exe| {
        let text = exe.to_string_lossy();
        if !text.contains(".app/Contents/MacOS/") {
            return None;
        }
        // 外側のアプリ(パスの最初の「.app」まで)を選ぶ
        let end = text.find(".app/")? + ".app".len();
        Some(std::path::PathBuf::from(&text[..end]))
    })
}

/// セッションのClaude Codeが動いているアプリのウィンドウを前に出す。前に出したアプリの名前を返す。
/// アプリの中の、そのセッションのタブまでは選べない(外から選ぶ方法が無い)。
pub fn bring_to_front(pid: u32) -> Option<String> {
    let chain = ancestors(pid);
    #[cfg(target_os = "macos")]
    {
        let exes: Vec<_> = chain.into_iter().map(|(_, exe)| exe).collect();
        let bundle = mac_app_bundle(&exes)?;
        let ok = std::process::Command::new("open")
            .arg("-a")
            .arg(&bundle)
            .status()
            .is_ok_and(|s| s.success());
        ok.then(|| bundle.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
    }
    #[cfg(windows)]
    {
        windows_focus::focus_first_window(&chain)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = chain;
        None
    }
}

#[cfg(windows)]
mod windows_focus {
    use std::path::PathBuf;
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow,
        ShowWindow, GW_OWNER, SW_RESTORE,
    };

    struct Search {
        pid: u32,
        found: HWND,
    }

    // 見えていて、ほかのウィンドウに持たれていない(=作業用の主な)ウィンドウを探す
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam as *mut Search);
        let mut owner_pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut owner_pid);
        if owner_pid == search.pid && IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null() {
            search.found = hwnd;
            return 0;
        }
        1
    }

    /// 自分に近い先祖から順に、ウィンドウを持つものを探して前に出す。エクスプローラーはタスクバーや
    /// デスクトップのウィンドウを持つので、そこまで来たら諦める(関係のないウィンドウを出さない)。
    pub fn focus_first_window(chain: &[(u32, PathBuf)]) -> Option<String> {
        for (pid, exe) in chain {
            let name = exe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if name.eq_ignore_ascii_case("explorer") {
                return None;
            }
            let mut search = Search { pid: *pid, found: std::ptr::null_mut() };
            unsafe {
                EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
                if search.found.is_null() {
                    continue;
                }
                if IsIconic(search.found) != 0 {
                    ShowWindow(search.found, SW_RESTORE);
                }
                SetForegroundWindow(search.found);
            }
            return Some(name);
        }
        None
    }
}

/// 知らせを押したとき: セッションのアプリを前に出し、出せたかを返す。出せなかったときは、
/// 呼び出し側が詩織の「Claude」の画面を開く。どちらになったかをログに書く。
pub fn focus_session(session_id: &str) -> bool {
    let dir = crate::project_root().join("data").join("claude-status");
    let local_host = if cfg!(windows) { "win" } else { "mac" };
    let app = local_pid(&dir, session_id, local_host, &is_claude_alive).and_then(bring_to_front);
    match &app {
        Some(name) => crate::shiori_log::info("CLAUDE", "セッションのアプリを前に出した", &[("アプリ", name)]),
        None => crate::shiori_log::info("CLAUDE", "セッションのアプリを前に出せなかった", &[("元", &"Claudeの画面へ")]),
    }
    app.is_some()
}

fn state_rank(state: &str) -> u8 {
    match state {
        "working" | "waiting" => 0,
        "idle" => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const HOUR: u64 = 60 * 60 * 1000;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("shiori-claude-status-{tag}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, id: &str, state: &str, updated_at: u64) {
        write_with(dir, id, state, updated_at, "mac", None);
    }

    fn write_with(dir: &Path, id: &str, state: &str, updated_at: u64, host: &str, pid: Option<u32>) {
        let pid = pid.map_or("null".to_string(), |p| p.to_string());
        let json = format!(
            r#"{{"session_id":"{id}","project":"p-{id}","title":"","host":"{host}","pid":{pid},"state":"{state}","started_at":0,"updated_at":{updated_at}}}"#
        );
        fs::write(dir.join(format!("{id}.json")), json).unwrap();
    }

    // pidが100のものだけ生きている、と仮定した判定。
    fn alive_100(pid: u32) -> bool {
        pid == 100
    }

    fn read(dir: &Path, now: u64) -> Vec<ClaudeSession> {
        read_sessions(dir, now, "mac", &alive_100)
    }

    fn states(sessions: &[ClaudeSession]) -> Vec<(&str, &str)> {
        sessions.iter().map(|s| (s.session_id.as_str(), s.state.as_str())).collect()
    }

    #[test]
    fn ディレクトリが無ければ空を返す() {
        assert!(read(Path::new("/nonexistent-shiori-dir"), 0).is_empty());
    }

    #[test]
    fn 状態の判定と並び順() {
        let dir = temp_dir("order");
        let now = 100 * HOUR;
        write(&dir, "idle1", "idle", now - 1_000);
        write(&dir, "work1", "working", now - 2_000);
        write(&dir, "wait1", "waiting", now - 500);
        write(&dir, "stale1", "working", now - STALE_AFTER_MS - 1);
        write(&dir, "end1", "ended", now - 1_000);
        assert_eq!(
            states(&read(&dir, now)),
            vec![
                ("wait1", "waiting"),
                ("work1", "working"),
                ("idle1", "idle"),
                ("end1", "ended"),
                ("stale1", "stale"),
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 生きているプロセスは長い無音でも作業中のまま() {
        let dir = temp_dir("alive");
        let now = 100 * HOUR;
        // 3時間更新が無くても、プロセスが生きていれば信じる(長いビルドや長い推論)。
        write_with(&dir, "long", "working", now - 3 * HOUR, "mac", Some(100));
        write_with(&dir, "longidle", "idle", now - 30 * HOUR, "mac", Some(100));
        assert_eq!(
            states(&read(&dir, now)),
            vec![("long", "working"), ("longidle", "idle")]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 死んだプロセスは終了扱いにして一定時間後に外す() {
        let dir = temp_dir("dead");
        let now = 100 * HOUR;
        write_with(&dir, "crash", "working", now - 1_000, "mac", Some(999));
        write_with(&dir, "oldcrash", "working", now - ENDED_VISIBLE_MS - 1, "mac", Some(999));
        assert_eq!(states(&read(&dir, now)), vec![("crash", "ended")]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 別機やpidなしは時間で判定する() {
        let dir = temp_dir("fallback");
        let now = 100 * HOUR;
        // 別のPC(win)のpidは、このPCでは確かめない(pidが偶然一致しても無視する)。
        write_with(&dir, "other", "working", now - 5 * 60 * 1000, "win", Some(100));
        write_with(&dir, "other-stale", "working", now - STALE_AFTER_MS - 1, "win", Some(999));
        write_with(&dir, "nopid-hidden", "idle", now - HIDE_AFTER_MS - 1, "mac", None);
        assert_eq!(
            states(&read(&dir, now)),
            vec![("other", "working"), ("other-stale", "stale")]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 古い終了は外す() {
        let dir = temp_dir("hide");
        let now = 100 * HOUR;
        write(&dir, "oldend", "ended", now - ENDED_VISIBLE_MS - 1);
        write(&dir, "keep", "idle", now - 60_000);
        let ids: Vec<String> = read(&dir, now).into_iter().map(|s| s.session_id).collect();
        assert_eq!(ids, vec!["keep".to_string()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 壊れたファイルと一時ファイルは無視する() {
        let dir = temp_dir("broken");
        let now = 1_000_000;
        fs::write(dir.join("bad.json"), "{not json").unwrap();
        fs::write(dir.join("s.json.123.tmp"), "{}").unwrap();
        fs::write(dir.join("._s.json"), [0u8, 1, 2]).unwrap();
        write(&dir, "ok", "idle", now);
        let ids: Vec<String> = read(&dir, now).into_iter().map(|s| s.session_id).collect();
        assert_eq!(ids, vec!["ok".to_string()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 自分自身のプロセスはclaudeではないので生きていると判定しない() {
        assert!(!is_claude_alive(std::process::id()));
    }

    #[test]
    fn 前に出すpidはこのPCの生きているセッションだけ() {
        let dir = temp_dir("localpid");
        write_with(&dir, "here", "idle", 0, "mac", Some(100));
        write_with(&dir, "dead", "idle", 0, "mac", Some(200));
        write_with(&dir, "other", "idle", 0, "win", Some(100));
        write_with(&dir, "nopid", "idle", 0, "mac", None);
        assert_eq!(local_pid(&dir, "here", "mac", &alive_100), Some(100));
        assert_eq!(local_pid(&dir, "dead", "mac", &alive_100), None);
        assert_eq!(local_pid(&dir, "other", "mac", &alive_100), None);
        assert_eq!(local_pid(&dir, "nopid", "mac", &alive_100), None);
        assert_eq!(local_pid(&dir, "missing", "mac", &alive_100), None);
        // パスとして働くidは、ファイルを探しに行かない
        assert_eq!(local_pid(&dir, "../here", "mac", &alive_100), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn macでは根に近いアプリを前に出す() {
        let p = |s: &str| std::path::PathBuf::from(s);
        // Claudeのデスクトップアプリ: 中のclaude.appではなく、Claude.appを選ぶ
        let desktop = [
            p("/Users/u/Library/Application Support/Claude/claude-code/2.1/x/claude.app/Contents/MacOS/claude"),
            p("/Applications/Claude.app/Contents/Helpers/disclaimer"),
            p("/Applications/Claude.app/Contents/MacOS/Claude"),
            p("/sbin/launchd"),
        ];
        assert_eq!(mac_app_bundle(&desktop), Some(p("/Applications/Claude.app")));
        // ターミナル
        let terminal = [
            p("/usr/local/bin/claude"),
            p("/bin/zsh"),
            p("/usr/bin/login"),
            p("/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"),
        ];
        assert_eq!(mac_app_bundle(&terminal), Some(p("/System/Applications/Utilities/Terminal.app")));
        // VS Codeの端末: 手伝いのアプリではなく、外側のCode.app
        let vscode = [
            p("/usr/local/bin/claude"),
            p("/bin/zsh"),
            p("/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin)"),
            p("/Applications/Visual Studio Code.app/Contents/MacOS/Electron"),
        ];
        assert_eq!(mac_app_bundle(&vscode), Some(p("/Applications/Visual Studio Code.app")));
        // アプリの中で動いていない(tmuxなど)
        assert_eq!(mac_app_bundle(&[p("/usr/local/bin/claude"), p("/opt/homebrew/bin/tmux")]), None);
    }
}
