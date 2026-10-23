//! 別のプログラムの拡張機能を起動し、見張る(詩織Ver6.0の段階1)。
//!
//! やり取りは標準入出力で、1行1つのJSON(JSON-RPCに近い形)。向きは両方。
//! - 拡張機能から詩織へ: `{"id": 1, "method": "log.write", "params": {...}}`
//!   → 詩織は`{"id": 1, "result": ...}`か`{"id": 1, "error": {"code": "EXT-011", "message": "..."}}`を返す
//! - 詩織から拡張機能へ(ほかの拡張機能からの呼び出し): `{"id": "h-7", "method": "greet", "params": {...}}`
//!   → 拡張機能が同じ形で返す
//! - 標準エラーに出たものは、その拡張機能の名前でログに書く
//!
//! 止まったら再起動する。60秒の間に3回止まったら、再起動をやめて「止まっている」にする(何度も落ちるものを
//! 回し続けないため)。詩織が落ちてもプログラムを残さないよう、Windowsはジョブオブジェクトに入れる。
//! Macでは「標準入力が閉じたら終わる」を拡張機能の決まりにする(詩織が落ちると標準入力も閉じる)。

use super::bridge::CallError;
use crate::error_codes;
use crate::shiori_log;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// 拡張機能からの頼みごとを処理するもの(`bridge::call`を、詩織の状態と一緒に包んだもの)。
pub type Handler = Arc<dyn Fn(&str, &str, &Value) -> Result<Value, CallError> + Send + Sync>;

/// 再起動をやめるまでの、止まった回数と時間の幅。
const GIVE_UP_EXITS: usize = 3;
const GIVE_UP_WINDOW: Duration = Duration::from_secs(60);
/// 止まってから再起動するまでの間。
const RESTART_DELAY: Duration = Duration::from_secs(1);
/// 詩織から頼んだときに、返事を待つ時間。
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum RunState {
    Starting,
    Running,
    Stopped { reason: String },
}

struct Slot {
    exe: PathBuf,
    dir: PathBuf,
    state: RunState,
    child: Option<Arc<Mutex<Child>>>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    exits: VecDeque<Instant>,
    restarts: u32,
    /// 詩織から頼んで、返事を待っているもの
    pending: HashMap<String, mpsc::Sender<Result<Value, CallError>>>,
    next_id: u64,
    /// 詩織が自分で止めたか(再起動しない)
    stopping: bool,
    /// 起動のたびに増やす。前の起動の見張りが、新しい起動に触らないようにする
    generation: u64,
}

#[derive(Clone)]
pub struct Programs {
    slots: Arc<Mutex<HashMap<String, Slot>>>,
    handler: Handler,
}

/// 詩織の管理する状態(`tauri::Manager::manage`で持つ)。
pub struct ProgramsState(pub Programs);

impl Programs {
    pub fn new(handler: Handler) -> Self {
        Self { slots: Arc::new(Mutex::new(HashMap::new())), handler }
    }

    /// 起動する(すでに動いていれば何もしない)。
    pub fn start(&self, name: &str, exe: PathBuf, dir: PathBuf) {
        {
            let mut slots = self.slots.lock().unwrap();
            if slots.get(name).is_some_and(|s| !matches!(s.state, RunState::Stopped { .. })) {
                return;
            }
            slots.insert(
                name.to_string(),
                Slot {
                    exe,
                    dir,
                    state: RunState::Starting,
                    child: None,
                    stdin: None,
                    exits: VecDeque::new(),
                    restarts: 0,
                    pending: HashMap::new(),
                    next_id: 1,
                    stopping: false,
                    generation: 0,
                },
            );
        }
        self.spawn(name);
    }

    fn spawn(&self, name: &str) {
        let (exe, dir, generation) = {
            let mut slots = self.slots.lock().unwrap();
            let Some(slot) = slots.get_mut(name) else { return };
            if slot.stopping {
                return;
            }
            slot.generation += 1;
            slot.state = RunState::Starting;
            (slot.exe.clone(), slot.dir.clone(), slot.generation)
        };
        let mut cmd = Command::new(&exe);
        cmd.current_dir(&dir)
            .env("SHIORI_EXTENSION_NAME", name)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::no_console_window(&mut cmd);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                shiori_log::error(
                    error_codes::EXT_018.code,
                    error_codes::EXT_018.message,
                    &[("name", &name), ("実行ファイル", &exe.display()), ("理由", &e)],
                );
                self.set_state(name, generation, RunState::Stopped { reason: format!("起動できない: {e}") });
                return;
            }
        };
        job::assign(&child);
        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = child.stdin.take().map(|s| Arc::new(Mutex::new(s)));
        let child = Arc::new(Mutex::new(child));
        {
            let mut slots = self.slots.lock().unwrap();
            let Some(slot) = slots.get_mut(name) else { return };
            slot.child = Some(child.clone());
            slot.stdin = stdin.clone();
            slot.state = RunState::Running;
        }
        shiori_log::info("HUB", "別のプログラムを起動した", &[("name", &name), ("pid", &pid)]);

        if let (Some(out), Some(stdin)) = (stdout, stdin) {
            let this = self.clone();
            let name = name.to_string();
            std::thread::spawn(move || this.read_stdout(&name, out, stdin));
        }
        if let Some(err) = stderr {
            let name = name.to_string();
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    shiori_log::warn(&name, "標準エラー", &[("内容", &line)]);
                }
            });
        }
        let this = self.clone();
        let name = name.to_string();
        std::thread::spawn(move || this.watch(&name, child, generation));
    }

    fn set_state(&self, name: &str, generation: u64, state: RunState) {
        if let Some(slot) = self.slots.lock().unwrap().get_mut(name) {
            if slot.generation == generation {
                slot.state = state;
            }
        }
    }

    /// 標準出力の行を読む。頼みごとは窓口に通し(1つずつ別のスレッドで。相手を待たせないため)、
    /// 返事は、待っている詩織の側に渡す。
    fn read_stdout(&self, name: &str, out: std::process::ChildStdout, stdin: Arc<Mutex<ChildStdin>>) {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                let head: String = line.chars().take(200).collect();
                shiori_log::warn(error_codes::EXT_019.code, error_codes::EXT_019.message, &[("name", &name), ("行", &head)]);
                continue;
            };
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            if let Some(method) = msg.get("method").and_then(Value::as_str) {
                let handler = self.handler.clone();
                let stdin = stdin.clone();
                let name = name.to_string();
                let method = method.to_string();
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                std::thread::spawn(move || {
                    let reply = match handler(&name, &method, &params) {
                        Ok(result) => serde_json::json!({ "id": id, "result": result }),
                        Err(e) => serde_json::json!({ "id": id, "error": e }),
                    };
                    write_line(&stdin, &reply);
                });
            } else if let Some(key) = id.as_str() {
                let sender = self.slots.lock().unwrap().get_mut(name).and_then(|s| s.pending.remove(key));
                if let Some(sender) = sender {
                    let result = match msg.get("error") {
                        Some(err) => Err(CallError {
                            code: leak_code(err.get("code").and_then(Value::as_str).unwrap_or("")),
                            message: err.get("message").and_then(Value::as_str).unwrap_or("").to_string(),
                        }),
                        None => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = sender.send(result);
                }
            } else {
                shiori_log::warn(error_codes::EXT_019.code, error_codes::EXT_019.message, &[("name", &name), ("行", &"methodもidも無い")]);
            }
        }
    }

    /// 止まるのを待ち、止まったら、決まりに従って再起動するか、やめる。
    fn watch(&self, name: &str, child: Arc<Mutex<Child>>, generation: u64) {
        let status = loop {
            if let Ok(Some(status)) = child.lock().unwrap().try_wait() {
                break status;
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        let restart = {
            let mut slots = self.slots.lock().unwrap();
            let Some(slot) = slots.get_mut(name) else { return };
            if slot.generation != generation {
                return;
            }
            slot.child = None;
            slot.stdin = None;
            for (_, sender) in slot.pending.drain() {
                let _ = sender.send(Err(CallError::new(error_codes::EXT_017, "返事の前に止まった")));
            }
            if slot.stopping {
                slot.state = RunState::Stopped { reason: "詩織が止めた".into() };
                shiori_log::info("HUB", "別のプログラムを止めた", &[("name", &name)]);
                false
            } else if record_exit(&mut slot.exits, Instant::now()) {
                slot.state = RunState::Stopped { reason: "60秒に3回止まったので、再起動をやめた".into() };
                shiori_log::error(
                    error_codes::EXT_016.code,
                    error_codes::EXT_016.message,
                    &[("name", &name), ("終了コード", &status)],
                );
                false
            } else {
                slot.state = RunState::Starting;
                slot.restarts += 1;
                shiori_log::warn(
                    error_codes::EXT_015.code,
                    error_codes::EXT_015.message,
                    &[("name", &name), ("終了コード", &status), ("再起動", &slot.restarts)],
                );
                true
            }
        };
        if restart {
            std::thread::sleep(RESTART_DELAY);
            self.spawn(name);
        }
    }

    /// 詩織から拡張機能に頼む(ほかの拡張機能からの呼び出しの中継に使う)。
    pub fn call(&self, name: &str, method: &str, params: &Value, timeout: Duration) -> Result<Value, CallError> {
        let (id, stdin, rx) = {
            let mut slots = self.slots.lock().unwrap();
            let slot = slots
                .get_mut(name)
                .filter(|s| s.state == RunState::Running)
                .ok_or_else(|| CallError::new(error_codes::EXT_017, format!("{name}は動いていない")))?;
            let stdin = slot.stdin.clone().ok_or_else(|| CallError::new(error_codes::EXT_017, "標準入力が無い"))?;
            let id = format!("h-{}", slot.next_id);
            slot.next_id += 1;
            let (tx, rx) = mpsc::channel();
            slot.pending.insert(id.clone(), tx);
            (id, stdin, rx)
        };
        write_line(&stdin, &serde_json::json!({ "id": id, "method": method, "params": params }));
        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                if let Some(slot) = self.slots.lock().unwrap().get_mut(name) {
                    slot.pending.remove(&id);
                }
                Err(CallError::new(error_codes::EXT_017, format!("{}秒で返事が無い", timeout.as_secs())))
            }
        }
    }

    /// いまの状態と、再起動した回数。
    pub fn state_of(&self, name: &str) -> Option<(RunState, u32)> {
        self.slots.lock().unwrap().get(name).map(|s| (s.state.clone(), s.restarts))
    }

    /// 全部を止める(詩織を閉じるとき)。
    pub fn stop_all(&self) {
        let children: Vec<(String, Arc<Mutex<Child>>)> = {
            let mut slots = self.slots.lock().unwrap();
            slots
                .iter_mut()
                .filter_map(|(name, slot)| {
                    slot.stopping = true;
                    // 標準入力を閉じて、自分で終わる機会を与える
                    slot.stdin = None;
                    slot.child.clone().map(|c| (name.clone(), c))
                })
                .collect()
        };
        for (name, child) in children {
            let _ = child.lock().unwrap().kill();
            shiori_log::info("HUB", "別のプログラムを終わらせた", &[("name", &name)]);
        }
    }
}

fn write_line(stdin: &Arc<Mutex<ChildStdin>>, msg: &Value) {
    let mut line = msg.to_string();
    line.push('\n');
    if let Ok(mut w) = stdin.lock() {
        let _ = w.write_all(line.as_bytes());
        let _ = w.flush();
    }
}

/// 拡張機能から届いたエラーコードを、`CallError`の`&'static str`に入れる。
/// よく使うコードは表から引き、それ以外は拡張機能のコードとして残す(数は拡張機能のエラーの種類の分だけ)。
fn leak_code(code: &str) -> &'static str {
    if let Some(known) = error_codes::lookup(code) {
        return known.code;
    }
    static SEEN: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap();
    if let Some(c) = seen.iter().find(|c| **c == code) {
        return c;
    }
    let c: &'static str = Box::leak(code.to_string().into_boxed_str());
    seen.push(c);
    c
}

/// 止まったことを記録し、再起動をやめるべきかを返す(60秒の間に3回)。
fn record_exit(exits: &mut VecDeque<Instant>, now: Instant) -> bool {
    exits.push_back(now);
    while exits.front().is_some_and(|t| now.duration_since(*t) > GIVE_UP_WINDOW) {
        exits.pop_front();
    }
    exits.len() >= GIVE_UP_EXITS
}

/// Windowsのジョブオブジェクト。詩織が落ちると、OSがジョブの中のプロセスをまとめて止める。
#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    // HANDLEはポインタなのでスレッドをまたげない。数として持つ(閉じずに、詩織が終わるまで持ち続ける)
    static JOB: OnceLock<usize> = OnceLock::new();

    fn job() -> usize {
        *JOB.get_or_init(|| unsafe {
            // SAFETY: 名前の無いジョブを作り、閉じたら中のプロセスを止める設定にする
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            handle as usize
        })
    }

    pub fn assign(child: &std::process::Child) {
        let job = job();
        if job == 0 {
            crate::shiori_log::warn("HUB", "ジョブオブジェクトを作れない(詩織が落ちると、別のプログラムが残るかもしれない)", &[]);
            return;
        }
        // SAFETY: 起動したばかりの子プロセスのハンドルを、作ったジョブに入れる
        let ok = unsafe { AssignProcessToJobObject(job as _, child.as_raw_handle() as _) };
        if ok == 0 {
            crate::shiori_log::warn("HUB", "別のプログラムをジョブに入れられない", &[("pid", &child.id())]);
        }
    }
}

#[cfg(not(windows))]
mod job {
    /// Macでは、標準入力が閉じたら終わる決まりで残さない(詩織が落ちると、標準入力も閉じる)。
    pub fn assign(_child: &std::process::Child) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 六十秒に三回止まったら再起動をやめる() {
        let t0 = Instant::now();
        let mut exits = VecDeque::new();
        assert!(!record_exit(&mut exits, t0));
        assert!(!record_exit(&mut exits, t0 + Duration::from_secs(10)));
        assert!(record_exit(&mut exits, t0 + Duration::from_secs(20)));
    }

    #[test]
    fn 間が空けば数え直す() {
        let t0 = Instant::now();
        let mut exits = VecDeque::new();
        assert!(!record_exit(&mut exits, t0));
        assert!(!record_exit(&mut exits, t0 + Duration::from_secs(50)));
        // 最初の1回は70秒前なので数えない(残るのは50秒と70秒の2回)
        assert!(!record_exit(&mut exits, t0 + Duration::from_secs(70)));
        assert!(record_exit(&mut exits, t0 + Duration::from_secs(80)));
    }

    #[test]
    fn 拡張機能のエラーコードを残す() {
        assert_eq!(leak_code("EXT-011"), "EXT-011");
        let a = leak_code("hello-program-001");
        let b = leak_code("hello-program-001");
        assert_eq!(a, "hello-program-001");
        assert!(std::ptr::eq(a, b));
    }
}
