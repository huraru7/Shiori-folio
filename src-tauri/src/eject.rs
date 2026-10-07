//! 実機SSDを安全に取り外せる状態にするための、プロセスの選択と停止(詩織の取り外し機能)。
//!
//! 止める対象は、名前(llama-server・uvicorn等)ではなく**実行ファイルのパスがSSD(portable/)
//! 配下にある、自分のユーザーのプロセス**で選ぶ。プロセスが増えても手直しが要らず、SSDの外の
//! 同名プロセスを誤って止めない。
//!
//! `mcp_server`も止める(取り外しを妨げるため)。Claudeのセッションが標準入出力でつないで使っており、
//! 外から止めるとそのセッションのMCP接続は復旧しない(decision-1790068500)。そのため止める前の
//! 確認に、使っているClaudeのセッションを案内として載せる。取り外し後にMCPを使うにはセッションの開き直しが要る。
//!
//! 選択([`build_plan`])は、プロセス一覧を渡すだけの純粋な関数にしてあり、単体テストできる。

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 穏やかな停止を待つ時間。これを過ぎても残っていれば強制終了する。
const GRACEFUL_WAIT: Duration = Duration::from_secs(10);
/// `lsof`(Mac/Linux)でSSDを開いているほかのプロセスを探す時間の上限。
const LSOF_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub exe: Option<PathBuf>,
    pub user: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub pid: u32,
    /// 親プロセスのpid(分かれば)。詩織を閉じたあとも残るもの(詩織の子ではないもの)の判定に使う。
    pub parent_pid: Option<u32>,
    pub name: String,
    pub exe: String,
    /// 止めたときの影響の説明(何がそれを使っているか等)。特に注意が要るものだけに付く。
    pub note: String,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// 止める予定のもの(停止の順に並ぶ)。
    pub to_stop: Vec<Entry>,
}

fn entry(p: &ProcInfo, note: &str) -> Entry {
    Entry {
        pid: p.pid,
        parent_pid: p.ppid,
        name: p.name.clone(),
        exe: p.exe.as_ref().map(|e| e.display().to_string()).unwrap_or_default(),
        note: note.to_string(),
    }
}

fn stem(p: &ProcInfo) -> String {
    p.exe
        .as_ref()
        .and_then(|e| e.file_stem())
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| p.name.to_lowercase().trim_end_matches(".exe").to_string())
}

fn is_mcp_server(p: &ProcInfo) -> bool {
    stem(p) == "mcp_server"
}

/// 止める順の優先度。詩織本体→RAGのpython→それ以外。
fn stop_rank(p: &ProcInfo) -> u8 {
    let s = stem(p);
    if s == "shiori-folio" {
        0
    } else if s.starts_with("python") || s == "uvicorn" {
        1
    } else {
        2
    }
}

/// `pid`と、その親をたどった祖先のpid(自分自身を含む)。
fn with_ancestors(by_pid: &HashMap<u32, &ProcInfo>, pid: u32) -> Vec<u32> {
    let mut chain = vec![pid];
    let mut cur = pid;
    while let Some(ppid) = by_pid.get(&cur).and_then(|p| p.ppid) {
        if chain.contains(&ppid) || ppid == 0 {
            break;
        }
        chain.push(ppid);
        cur = ppid;
    }
    chain
}

/// プロセス一覧から、止めるものを選ぶ。
///
/// - `root`: SSD(portable/)のルート。実行ファイルがこの配下にあるプロセスだけが対象。
/// - `self_user`: このプロセスのユーザー。ほかのユーザーのプロセスは対象外。
/// - `self_pid`: このプロセス。自分自身と、その親をたどった祖先(CLIを呼んだシェル・Claude等)は止めない。
pub fn build_plan(procs: &[ProcInfo], root: &Path, self_user: Option<&str>, self_pid: u32) -> Plan {
    let by_pid: HashMap<u32, &ProcInfo> = procs.iter().map(|p| (p.pid, p)).collect();
    let protected: Vec<u32> = with_ancestors(&by_pid, self_pid);

    let mut to_stop: Vec<&ProcInfo> = Vec::new();
    for p in procs {
        let Some(exe) = &p.exe else { continue };
        if !exe.starts_with(root) || protected.contains(&p.pid) {
            continue;
        }
        if let (Some(me), Some(owner)) = (self_user, p.user.as_deref()) {
            if me != owner {
                continue;
            }
        }
        to_stop.push(p);
    }
    to_stop.sort_by_key(|p| (stop_rank(p), p.pid));
    Plan {
        to_stop: to_stop
            .into_iter()
            .map(|p| {
                let note = if is_mcp_server(p) { mcp_note(&by_pid, p) } else { String::new() };
                entry(p, &note)
            })
            .collect(),
    }
}

/// `mcp_server`を止めたときの影響の説明。動かしているClaude(親をたどった、SSDの外のプロセス)を含める。
fn mcp_note(by_pid: &HashMap<u32, &ProcInfo>, p: &ProcInfo) -> String {
    let holders: Vec<String> = with_ancestors(by_pid, p.pid)
        .into_iter()
        .skip(1)
        .filter_map(|pid| by_pid.get(&pid))
        .take(3)
        .map(|a| format!("{}(pid {})", a.name, a.pid))
        .collect();
    if holders.is_empty() {
        "MCPサーバー。Claudeのセッションが使用中。止めると、そのセッションのMCPは使えなくなる".to_string()
    } else {
        format!(
            "MCPサーバー。Claudeのセッションが使用中(親: {})。止めると、そのセッションのMCPは使えなくなる",
            holders.join(" ← ")
        )
    }
}

/// 実行中のプロセスの一覧を集める。実行ファイルのパスは、シンボリックリンクを解決した形にそろえる。
pub fn snapshot(sys: &sysinfo::System) -> Vec<ProcInfo> {
    sys.processes()
        .iter()
        .map(|(pid, p)| ProcInfo {
            pid: pid.as_u32(),
            ppid: p.parent().map(|x| x.as_u32()),
            name: p.name().to_string_lossy().to_string(),
            exe: p.exe().map(|e| e.canonicalize().unwrap_or_else(|_| e.to_path_buf())),
            user: p.user_id().map(|u| u.to_string()),
        })
        .collect()
}

pub fn refreshed_system() -> sysinfo::System {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new()
            .with_exe(UpdateKind::Always)
            .with_user(UpdateKind::Always),
    );
    sys
}

/// いまのプロセス一覧から、このPCの現在の計画を作る。`root`は、シンボリックリンクを解決して比べる。
pub fn current_plan(root: &Path) -> (Plan, sysinfo::System) {
    let sys = refreshed_system();
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let self_pid = std::process::id();
    let procs = snapshot(&sys);
    let self_user = procs.iter().find(|p| p.pid == self_pid).and_then(|p| p.user.clone());
    let plan = build_plan(&procs, &root, self_user.as_deref(), self_pid);
    (plan, sys)
}

#[derive(Serialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StopOutcome {
    /// 穏やかな停止で止まったpid。
    pub stopped: Vec<u32>,
    /// 待っても止まらず、強制終了したpid。
    pub forced: Vec<u32>,
    /// 強制終了もできなかった(または終了を確認できなかった)pid。
    pub failed: Vec<u32>,
}

/// 終了済みで、親に回収されていない(ゾンビ)か。unixのみ。
/// sysinfoはmacOSでゾンビを更新できず、古い「実行中」の情報のまま残すため、`ps`で状態を直接見る。
#[cfg(unix)]
fn is_zombie(pid: u32) -> bool {
    std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim_start().starts_with('Z'))
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_zombie(_pid: u32) -> bool {
    false
}

/// 生きているプロセスか。ゾンビは、止まったものとして扱う。
fn is_alive(sys: &sysinfo::System, pid: sysinfo::Pid) -> bool {
    sys.process(pid).is_some() && !is_zombie(pid.as_u32())
}

/// `pids`を停止する。unixは`SIGTERM`で穏やかに止め、`GRACEFUL_WAIT`待っても残るものは強制終了する。
/// Windowsは穏やかな停止の手段がないため、強制終了のみ。
pub fn stop_processes(pids: &[u32]) -> StopOutcome {
    use sysinfo::{Pid, ProcessesToUpdate, Signal, System};
    let mut outcome = StopOutcome::default();
    if pids.is_empty() {
        return outcome;
    }
    let targets: Vec<Pid> = pids.iter().map(|p| Pid::from_u32(*p)).collect();
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::Some(&targets), true);

    for pid in &targets {
        if let Some(p) = sys.process(*pid) {
            // unixは穏やかな停止を送る。送れなかった(未対応のOS)ときは強制終了に回す。
            if p.kill_with(Signal::Term).is_none() {
                p.kill();
            }
        }
    }

    let deadline = Instant::now() + GRACEFUL_WAIT;
    loop {
        sys.refresh_processes(ProcessesToUpdate::Some(&targets), true);
        if targets.iter().all(|p| !is_alive(&sys, *p)) || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    for (pid, raw) in targets.iter().zip(pids) {
        if !is_alive(&sys, *pid) {
            outcome.stopped.push(*raw);
        } else if let Some(p) = sys.process(*pid) {
            p.kill();
            outcome.forced.push(*raw);
        }
    }
    // 強制終了したものが本当に消えたかを確かめる。
    if !outcome.forced.is_empty() {
        let forced: Vec<Pid> = outcome.forced.iter().map(|p| Pid::from_u32(*p)).collect();
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            sys.refresh_processes(ProcessesToUpdate::Some(&forced), true);
            if forced.iter().all(|p| !is_alive(&sys, *p)) || Instant::now() >= until {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let (gone, alive): (Vec<u32>, Vec<u32>) = outcome
            .forced
            .iter()
            .partition(|raw| !is_alive(&sys, Pid::from_u32(**raw)));
        outcome.forced = gone;
        outcome.failed = alive;
    }
    outcome
}

/// SSD配下のファイルを開いている、ほかのプロセス(エディタ等)を`lsof`で探す(Mac/Linuxのみ)。
/// 止めはせず、報告にだけ使う。`skip`は、止める予定・使用中として別に挙げたpid。
/// `lsof`が無い・遅すぎる環境では、空を返す。
pub fn other_holders(root: &Path, skip: &[u32]) -> Vec<(u32, String)> {
    if cfg!(windows) {
        return Vec::new();
    }
    let Ok(mut child) = std::process::Command::new("lsof")
        .arg("-F")
        .arg("pc")
        .arg("+D")
        .arg(root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .spawn()
    else {
        return Vec::new();
    };
    let deadline = Instant::now() + LSOF_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                break None;
            }
        }
    };
    let _ = status;
    let mut out = String::new();
    if let Some(mut so) = child.stdout.take() {
        use std::io::Read;
        let _ = so.read_to_string(&mut out);
    }
    parse_lsof(&out, std::process::id(), skip)
}

/// `lsof -F pc`の出力(`p<pid>`の行の後に`c<コマンド名>`の行が続く)から、(pid, 名前)を取り出す。
pub fn parse_lsof(output: &str, self_pid: u32, skip: &[u32]) -> Vec<(u32, String)> {
    let mut result: Vec<(u32, String)> = Vec::new();
    let mut current: Option<u32> = None;
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            current = rest.parse().ok();
        } else if let (Some(rest), Some(pid)) = (line.strip_prefix('c'), current) {
            if pid != self_pid && !skip.contains(&pid) && !result.iter().any(|(p, _)| *p == pid) {
                result.push((pid, rest.to_string()));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/Volumes/SSD/portable";

    fn p(pid: u32, ppid: Option<u32>, exe: Option<&str>, user: &str) -> ProcInfo {
        let exe_path = exe.map(PathBuf::from);
        ProcInfo {
            pid,
            ppid,
            name: exe_path
                .as_ref()
                .and_then(|e| e.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("proc{pid}")),
            exe: exe_path,
            user: Some(user.to_string()),
        }
    }

    fn pids(entries: &[Entry]) -> Vec<u32> {
        entries.iter().map(|e| e.pid).collect()
    }

    #[test]
    fn ssd配下だけを選び_外のプロセスは対象外() {
        let procs = vec![
            p(10, Some(1), Some("/Volumes/SSD/portable/bin/mac/llama-server"), "me"),
            p(11, Some(1), Some("/opt/homebrew/bin/llama-server"), "me"), // 同名でもSSDの外
            p(12, Some(1), None, "me"),                                   // 実行ファイルが分からない
        ];
        let plan = build_plan(&procs, Path::new(ROOT), Some("me"), 999);
        assert_eq!(pids(&plan.to_stop), vec![10]);
    }

    #[test]
    fn ほかのユーザーのプロセスは対象外() {
        let procs = vec![
            p(10, Some(1), Some("/Volumes/SSD/portable/bin/mac/llama-server"), "me"),
            p(11, Some(1), Some("/Volumes/SSD/portable/bin/mac/llama-server"), "other"),
        ];
        let plan = build_plan(&procs, Path::new(ROOT), Some("me"), 999);
        assert_eq!(pids(&plan.to_stop), vec![10]);
    }

    #[test]
    fn 自分自身とその親は止めない() {
        let procs = vec![
            p(100, Some(1), Some("/Volumes/SSD/portable/bin/mac/shiori-save"), "me"), // 親(SSD配下の仮定)
            p(200, Some(100), Some("/Volumes/SSD/portable/bin/mac/shiori-eject"), "me"), // 自分
        ];
        let plan = build_plan(&procs, Path::new(ROOT), Some("me"), 200);
        assert!(plan.to_stop.is_empty());
    }

    #[test]
    fn mcp_serverも止める対象で親のclaudeを案内する() {
        let procs = vec![
            p(1, None, Some("/sbin/launchd"), "root"),
            p(50, Some(1), Some("/Users/me/claude-code/claude"), "me"),
            p(60, Some(50), Some("/Volumes/SSD/portable/bin/mac/mcp_server"), "me"),
            p(70, Some(60), Some("/Volumes/SSD/portable/bin/mac/llama-server"), "me"),
        ];
        let plan = build_plan(&procs, Path::new(ROOT), Some("me"), 999);
        assert_eq!(pids(&plan.to_stop), vec![60, 70]);
        let mcp = plan.to_stop.iter().find(|e| e.pid == 60).unwrap();
        assert!(mcp.note.contains("claude(pid 50)"), "{}", mcp.note);
        assert!(plan.to_stop.iter().find(|e| e.pid == 70).unwrap().note.is_empty());
    }

    #[test]
    fn 別のセッションのmcp_serverもすべて止める対象にする() {
        let procs = vec![
            p(60, Some(50), Some("/Volumes/SSD/portable/bin/mac/mcp_server"), "me"),
            p(61, Some(51), Some("/Volumes/SSD/portable/bin/win/mcp_server.exe"), "me"),
        ];
        let plan = build_plan(&procs, Path::new(ROOT), Some("me"), 999);
        assert_eq!(pids(&plan.to_stop), vec![60, 61]);
    }

    #[test]
    fn 停止の順は詩織本体_rag_それ以外() {
        let procs = vec![
            p(30, Some(1), Some("/Volumes/SSD/portable/bin/mac/llama-server"), "me"),
            p(20, Some(1), Some("/Volumes/SSD/portable/bin/mac/rag-venv/bin/python3.12"), "me"),
            p(10, Some(1), Some("/Volumes/SSD/portable/bin/mac/shiori-folio.app/Contents/MacOS/shiori-folio"), "me"),
            p(40, Some(1), Some("/Volumes/SSD/portable/bin/mac/whisper-server"), "me"),
        ];
        let plan = build_plan(&procs, Path::new(ROOT), Some("me"), 999);
        assert_eq!(pids(&plan.to_stop), vec![10, 20, 30, 40]);
    }

    #[test]
    fn 止めるものが無ければ空() {
        let procs = vec![p(1, None, Some("/sbin/launchd"), "root")];
        assert_eq!(build_plan(&procs, Path::new(ROOT), Some("me"), 999), Plan::default());
    }

    #[test]
    fn ユーザーが分からなければ絞り込まない() {
        let procs = vec![p(10, Some(1), Some("/Volumes/SSD/portable/bin/mac/llama-server"), "other")];
        assert_eq!(pids(&build_plan(&procs, Path::new(ROOT), None, 999).to_stop), vec![10]);
    }

    #[test]
    fn lsofの出力を読む() {
        let out = "p100\ncnode\np200\ncVivaldi\np300\ncllama-ser\np200\ncVivaldi\n";
        let r = parse_lsof(out, 100, &[300]);
        assert_eq!(r, vec![(200, "Vivaldi".to_string())]);
    }

    #[test]
    fn 実際のプロセスを穏やかに止める() {
        // sleepを起動して、SIGTERMで止まることを確かめる(unixのみ)。
        if cfg!(windows) {
            return;
        }
        let mut child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
        let pid = child.id();
        // 親(このテスト)が終了を回収しないと、止まったプロセスがゾンビとして一覧に残るため、別スレッドで回収する。
        let reaper = std::thread::spawn(move || child.wait());
        let outcome = stop_processes(&[pid]);
        let _ = reaper.join();
        assert_eq!(outcome.stopped, vec![pid]);
        assert!(outcome.forced.is_empty() && outcome.failed.is_empty());
    }

    #[test]
    fn 止めたあと親に回収されないゾンビは止まったものとして扱う() {
        if cfg!(windows) {
            return;
        }
        // 親(このテスト)が終了を回収しないと、SIGTERMで止まったプロセスはゾンビとして残る。
        let child = std::process::Command::new("sleep").arg("60").spawn().unwrap();
        let pid = child.id();
        let outcome = stop_processes(&[pid]);
        assert_eq!(outcome.stopped, vec![pid]);
        assert!(outcome.forced.is_empty() && outcome.failed.is_empty());
        drop(child);
    }

    #[test]
    fn 存在しないpidは止めた扱いで空は何もしない() {
        assert_eq!(stop_processes(&[]), StopOutcome::default());
        let outcome = stop_processes(&[4_000_000]);
        assert_eq!(outcome.stopped, vec![4_000_000]);
    }
}
