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
}
