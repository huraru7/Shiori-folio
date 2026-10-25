//! 知らせと時刻の予約(詩織Ver6.0の段階3)。
//!
//! 知らせの種類(タイマーの終わり・Claudeの作業の終わり・要確認・拡張機能からの知らせ)は、全部
//! `show` の1つの入口を通る。ここでログに `[NOTIFY]` で書き、画面へ `shiori://notify` を送り、
//! 詩織のウィンドウが前に無ければタスクバー(MacはDock)で知らせる。札の出し方と音は画面の側が決める。
//! 段階4のペットの吹き出しも、同じ入口から出す。
//!
//! 予約は本体の中だけに持ち、ファイルには書かない(詩織を閉じたら消える)。1秒ごとに壁の時計と
//! 比べるので、PCが眠っていた間に時間が来たものは、起きたときにすぐ知らせる。
//! ログには予約の時刻と名前を書き、ラベルや本文は書かない(会話の中身を残さないのと同じ考え)。

use crate::error_codes;
use crate::hub::bridge::CallError;
use crate::shiori_log;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const PART: &str = "NOTIFY";
/// 画面へ知らせを送るイベントの名前
pub const EVENT: &str = "shiori://notify";
/// 拡張機能ごとの予約の上限
const MAX_PER_OWNER: usize = 20;
/// 予約できるのは、いまから7日以内
const MAX_AHEAD_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// 拡張機能からの知らせの題と本文の長さの上限
const MAX_TEXT: usize = 200;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Timer,
    Claude,
    Pending,
    Extension,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Timer => "timer",
            Kind::Claude => "claude",
            Kind::Pending => "pending",
            Kind::Extension => "extension",
        }
    }
}

/// 画面へ送る知らせ1つ。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub id: u64,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// 知らせの元の拡張機能の名前(本体の知らせは無い)
    pub source: Option<String>,
    pub at: u64,
}

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static NEXT_NOTICE: AtomicU64 = AtomicU64::new(1);

/// 画面(起動したあとだけある)。クリップボードもこれを通して書く。
pub fn app() -> Option<&'static tauri::AppHandle> {
    APP.get()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 知らせの入口。画面がまだ無いとき(テストなど)は、ログに書くだけ。
pub fn show(kind: Kind, title: &str, body: &str, source: Option<&str>) -> u64 {
    let id = NEXT_NOTICE.fetch_add(1, Ordering::Relaxed);
    let notice = Notice {
        id,
        kind,
        title: title.to_string(),
        body: body.to_string(),
        source: source.map(str::to_string),
        at: now_ms(),
    };
    let mut taskbar = "なし";
    if let Some(app) = APP.get() {
        use tauri::{Emitter, Manager};
        let _ = app.emit(EVENT, &notice);
        // ペットの形のときは、ペットの吹き出しで知らせる(本体のウィンドウは隠れているので点滅させない)
        if crate::pet::on_notice(&notice) {
            taskbar = "ペット";
        } else if let Some(window) = app.get_webview_window("main") {
            let front = window.is_focused().unwrap_or(false) && !window.is_minimized().unwrap_or(false);
            if !front {
                let _ = window.request_user_attention(Some(tauri::UserAttentionType::Informational));
                taskbar = "点滅";
            }
        }
    }
    shiori_log::info(
        PART,
        "知らせた",
        &[("種類", &kind.as_str()), ("元", &source.unwrap_or("本体")), ("id", &id), ("タスクバー", &taskbar)],
    );
    id
}

// ---- 時刻の予約 ----

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Reservation {
    pub id: u64,
    #[serde(skip)]
    pub owner: String,
    pub at: u64,
    pub label: String,
}

/// 予約の一覧(時計を外から渡すので、テストで時刻を決められる)。
#[derive(Default)]
pub struct Schedules {
    items: Vec<Reservation>,
    next_id: u64,
}

impl Schedules {
    pub fn add(&mut self, owner: &str, at: u64, label: &str, now: u64) -> Result<u64, CallError> {
        let bad = |d: &str| CallError::new(error_codes::EXT_014, d);
        if at <= now {
            return Err(bad("atが過去"));
        }
        if at - now > MAX_AHEAD_MS {
            return Err(bad("予約できるのは7日以内"));
        }
        if label.chars().count() > MAX_TEXT {
            return Err(bad("labelが長すぎる"));
        }
        if self.items.iter().filter(|r| r.owner == owner).count() >= MAX_PER_OWNER {
            return Err(CallError::new(error_codes::EXT_025, format!("{MAX_PER_OWNER}件まで")));
        }
        self.next_id += 1;
        let id = self.next_id;
        self.items.push(Reservation { id, owner: owner.to_string(), at, label: label.to_string() });
        Ok(id)
    }

    /// 自分の予約だけを消せる。ほかの拡張機能の予約や、もう終わった予約は `EXT-026`。
    pub fn cancel(&mut self, owner: &str, id: u64) -> Result<(), CallError> {
        let pos = self
            .items
            .iter()
            .position(|r| r.id == id && r.owner == owner)
            .ok_or_else(|| CallError::new(error_codes::EXT_026, id.to_string()))?;
        self.items.remove(pos);
        Ok(())
    }

    pub fn list(&self, owner: &str) -> Vec<Reservation> {
        let mut v: Vec<_> = self.items.iter().filter(|r| r.owner == owner).cloned().collect();
        v.sort_by_key(|r| r.at);
        v
    }

    /// 時間が来た予約を一覧から外して返す(早いものから)。
    pub fn take_due(&mut self, now: u64) -> Vec<Reservation> {
        let (mut due, rest): (Vec<_>, Vec<_>) = self.items.drain(..).partition(|r| r.at <= now);
        self.items = rest;
        due.sort_by_key(|r| r.at);
        due
    }
}

fn schedules() -> &'static Mutex<Schedules> {
    static S: OnceLock<Mutex<Schedules>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Schedules::default()))
}

fn lock() -> Result<std::sync::MutexGuard<'static, Schedules>, CallError> {
    schedules().lock().map_err(|e| CallError::new(error_codes::EXT_014, e.to_string()))
}

// ---- 窓口の頼みごと(bridge.rsから呼ぶ) ----

fn text_param(params: &Value, key: &str, required: bool) -> Result<String, CallError> {
    match params.get(key) {
        Some(Value::String(s)) if s.chars().count() <= MAX_TEXT => Ok(s.clone()),
        Some(Value::String(_)) => Err(CallError::new(error_codes::EXT_014, format!("{key}が長すぎる"))),
        None | Some(Value::Null) if !required => Ok(String::new()),
        _ => Err(CallError::new(error_codes::EXT_014, format!("{key}が要る(文字列)"))),
    }
}

fn id_param(params: &Value) -> Result<u64, CallError> {
    params.get("id").and_then(Value::as_u64).ok_or_else(|| CallError::new(error_codes::EXT_014, "idが要る"))
}

/// `notify.show {title, body}`
pub fn notify_show(name: &str, params: &Value) -> Result<Value, CallError> {
    let title = text_param(params, "title", true)?;
    let body = text_param(params, "body", false)?;
    let id = show(Kind::Extension, &title, &body, Some(name));
    Ok(serde_json::json!({ "id": id }))
}

/// `schedule.add {at, label}`
pub fn schedule_add(name: &str, params: &Value) -> Result<Value, CallError> {
    let at = params.get("at").and_then(Value::as_u64).ok_or_else(|| CallError::new(error_codes::EXT_014, "atが要る(ミリ秒)"))?;
    let label = text_param(params, "label", false)?;
    let id = lock()?.add(name, at, &label, now_ms())?;
    shiori_log::info(PART, "予約した", &[("name", &name), ("id", &id), ("残り秒", &((at.saturating_sub(now_ms())) / 1000))]);
    Ok(serde_json::json!({ "id": id }))
}

/// `schedule.cancel {id}`
pub fn schedule_cancel(name: &str, params: &Value) -> Result<Value, CallError> {
    let id = id_param(params)?;
    lock()?.cancel(name, id)?;
    shiori_log::info(PART, "予約を取り消した", &[("name", &name), ("id", &id)]);
    Ok(serde_json::json!({}))
}

/// `schedule.list {}`
pub fn schedule_list(name: &str) -> Result<Value, CallError> {
    serde_json::to_value(lock()?.list(name)).map_err(|e| CallError::new(error_codes::EXT_014, e.to_string()))
}

// ---- 見張り ----

/// 画面を覚え、予約とClaudeの見張りを始める(起動時に1回)。
pub fn start(app: tauri::AppHandle) {
    if APP.set(app).is_err() {
        return;
    }
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(1));
        let due = match schedules().lock() {
            Ok(mut s) => s.take_due(now_ms()),
            Err(_) => continue,
        };
        for r in due {
            let title = if r.label.is_empty() { "タイマー" } else { r.label.as_str() };
            show(Kind::Timer, title, "時間になりました", Some(&r.owner));
        }
    });
    std::thread::spawn(watch_claude);
}

/// Claude Codeのセッションが「作業中」から「待機」か「終了」に変わったら知らせる。
/// 起動したときにすでに終わっていたものは知らせない(最初の1回は覚えるだけ)。
fn watch_claude() {
    let dir = crate::project_root().join("data").join("claude-status");
    let local_host = if cfg!(windows) { "win" } else { "mac" };
    let mut prev: Option<HashMap<String, String>> = None;
    loop {
        let sessions = crate::claude_status::read_sessions(&dir, now_ms(), local_host, &crate::claude_status::is_claude_alive);
        let now: HashMap<String, String> = sessions.iter().map(|s| (s.session_id.clone(), s.state.clone())).collect();
        if let Some(prev) = &prev {
            for s in &sessions {
                let was_working = prev.get(&s.session_id).is_some_and(|st| st == "working");
                if was_working && (s.state == "idle" || s.state == "ended") {
                    let name = if s.title.is_empty() { s.project.as_str() } else { s.title.as_str() };
                    // 元にはセッションidを持たせ、押したときにそのセッションのアプリを前に出す
                    show(Kind::Claude, "Claudeの作業が終わった", name, Some(&s.session_id));
                }
            }
        }
        prev = Some(now);
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// 要確認の件数が増えたら知らせる。`count` は数え直す関数(起動したときの件数は知らせない)。
pub fn watch_pending(count: fn() -> Option<usize>) {
    std::thread::spawn(move || {
        let mut prev = count();
        loop {
            std::thread::sleep(Duration::from_secs(60));
            let Some(n) = count() else { continue };
            if let Some(p) = prev {
                if n > p {
                    show(Kind::Pending, "要確認が増えた", &format!("{}件(新しく{}件)", n, n - p), None);
                }
            }
            prev = Some(n);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 予約は時刻の順に来て_来たものは外れる() {
        let mut s = Schedules::default();
        let a = s.add("timer", 3_000, "b", 1_000).unwrap();
        let b = s.add("timer", 2_000, "a", 1_000).unwrap();
        assert_eq!(s.take_due(1_500), vec![]);
        let due = s.take_due(3_000);
        assert_eq!(due.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, a]);
        assert!(s.list("timer").is_empty());
    }

    #[test]
    fn 書き間違いと上限() {
        let mut s = Schedules::default();
        assert_eq!(s.add("t", 1_000, "", 1_000).unwrap_err().code, "EXT-014");
        assert_eq!(s.add("t", 1_000 + MAX_AHEAD_MS + 1, "", 1_000).unwrap_err().code, "EXT-014");
        for _ in 0..MAX_PER_OWNER {
            s.add("t", 5_000, "", 1_000).unwrap();
        }
        assert_eq!(s.add("t", 5_000, "", 1_000).unwrap_err().code, "EXT-025");
        // ほかの拡張機能は別に数える
        assert!(s.add("u", 5_000, "", 1_000).is_ok());
    }

    #[test]
    fn 自分の予約だけを取り消し_見られる() {
        let mut s = Schedules::default();
        let mine = s.add("timer", 5_000, "", 1_000).unwrap();
        let other = s.add("other", 5_000, "", 1_000).unwrap();
        assert_eq!(s.cancel("timer", other).unwrap_err().code, "EXT-026");
        assert_eq!(s.list("timer").len(), 1);
        s.cancel("timer", mine).unwrap();
        assert_eq!(s.cancel("timer", mine).unwrap_err().code, "EXT-026");
        assert_eq!(s.list("other").len(), 1);
    }
}
