//! ハブ(詩織Ver6.0の段階1)。拡張機能を読み込み、承認を取り、拡張機能と詩織の間の窓口になる。
//!
//! 拡張機能は `portable/extensions/{名前}/` に説明書(`extension.json`)と中身を置くだけで足せる
//! (詩織のビルドし直しは要らない)。詩織は起動時に全部のフォルダを読み、承認していないものは
//! 読み込まない。拡張機能どうしは直接やり取りせず、必ずハブを通す(構想メモ idea-shiori-ver6-20261022)。

pub mod approvals;
pub mod bridge;
pub mod manifest;
pub mod process;
pub mod protocol;
pub mod storage;

use crate::error_codes::{self, ErrorCode};
use crate::shiori_log;
use approvals::{Approval, ApprovalFile};
use bridge::CallError;
use manifest::{Kind, Manifest, Place};
use process::{ProgramsState, RunState};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 拡張機能1つの状態。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// 承認済みで、読み込んだ
    Ready,
    /// 承認を待っている。`added`は、承認の記録に無い使うもの(初めてのときは`uses`の全部)
    NeedsApproval { first_time: bool, added: Vec<String> },
    /// 説明書が読めないので、読み込まない
    Invalid { code: ErrorCode, detail: String },
}

#[derive(Clone, Debug)]
pub struct Extension {
    pub folder: String,
    pub dir: PathBuf,
    pub manifest: Option<Manifest>,
    pub status: Status,
}

pub struct Hub {
    extensions_dir: PathBuf,
    data_dir: PathBuf,
    approvals: ApprovalFile,
    extensions: Vec<Extension>,
}

/// 詩織の管理する状態(`tauri::Manager::manage`で持つ)。
pub struct HubState(pub Mutex<Hub>);

impl Hub {
    /// `root`(`portable/`)の下の拡張機能を読み込む。
    pub fn load(root: &Path) -> Self {
        Self::load_from(root.join("extensions"), root.join("data").join("extensions"))
    }

    pub fn load_from(extensions_dir: PathBuf, data_dir: PathBuf) -> Self {
        let approvals_path = approvals::approvals_path(&data_dir);
        let approvals = match approvals::load(&approvals_path) {
            Ok(a) => a,
            Err(detail) => {
                // 壊れた記録は脇へ移して残し、全部を「承認待ち」として扱う(勝手に許さない側へ倒す)
                let aside = approvals::set_aside_broken(&approvals_path);
                shiori_log::error(
                    error_codes::EXT_007.code,
                    "承認の記録が読めないので、全部を承認待ちにする",
                    &[("理由", &detail), ("移した先", &aside.map(|p| p.display().to_string()).unwrap_or_default())],
                );
                ApprovalFile { format: 1, ..Default::default() }
            }
        };
        let mut hub = Self { extensions_dir, data_dir, approvals, extensions: Vec::new() };
        hub.scan();
        hub
    }

    /// 拡張機能のフォルダを全部読み直す。
    fn scan(&mut self) {
        self.extensions.clear();
        let entries = match std::fs::read_dir(&self.extensions_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                shiori_log::info("HUB", "拡張機能のフォルダが無い", &[("場所", &self.extensions_dir.display())]);
                return;
            }
            Err(e) => {
                shiori_log::error(
                    error_codes::HUB_001.code,
                    error_codes::HUB_001.message,
                    &[("場所", &self.extensions_dir.display()), ("理由", &e)],
                );
                return;
            }
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            // Macの`._`ファイルや、`_`で始まる作業用のフォルダは拡張機能として扱わない
            .filter(|p| p.file_name().is_some_and(|n| !n.to_string_lossy().starts_with(['.', '_'])))
            .collect();
        dirs.sort();
        for dir in dirs {
            let ext = self.read_one(dir);
            log_extension(&ext);
            self.extensions.push(ext);
        }
        let ready = self.extensions.iter().filter(|e| e.status == Status::Ready).count();
        shiori_log::info(
            "HUB",
            "拡張機能を読み込んだ",
            &[("数", &self.extensions.len()), ("使える", &ready), ("場所", &self.extensions_dir.display())],
        );
    }

    fn read_one(&self, dir: PathBuf) -> Extension {
        let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (manifest, status) = match manifest::load(&dir) {
            Ok(m) => {
                let status = self.approval_status(&m);
                (Some(m), status)
            }
            Err(e) => (None, Status::Invalid { code: e.code, detail: e.detail }),
        };
        Extension { folder, dir, manifest, status }
    }

    fn approval_status(&self, m: &Manifest) -> Status {
        match approvals::unapproved_uses(&m.uses, self.approvals.extensions.get(&m.name)) {
            None => Status::NeedsApproval { first_time: true, added: m.uses.clone() },
            Some(added) if added.is_empty() => Status::Ready,
            Some(added) => Status::NeedsApproval { first_time: false, added },
        }
    }

    pub fn extensions(&self) -> &[Extension] {
        &self.extensions
    }

    /// 承認済みで読み込んだ拡張機能を、名前で引く。
    pub fn ready(&self, name: &str) -> Option<&Extension> {
        self.extensions.iter().find(|e| e.folder == name && e.status == Status::Ready)
    }

    /// 拡張機能ごとの置き場(`portable/data/extensions/{名前}/`)。
    pub fn data_dir_of(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    /// 承認する。説明書のいまの`uses`を、承認の記録に残す。
    pub fn approve(&mut self, name: &str) -> Result<(), (ErrorCode, String)> {
        let index = self
            .extensions
            .iter()
            .position(|e| e.folder == name && e.manifest.is_some())
            .ok_or_else(|| (error_codes::EXT_008, name.to_string()))?;
        let manifest = self.extensions[index].manifest.clone().unwrap();
        let mut next = self.approvals.clone();
        next.extensions.insert(
            manifest.name.clone(),
            Approval {
                version: manifest.version.clone(),
                uses: manifest.uses.clone(),
                approved_at: chrono::Local::now().to_rfc3339(),
            },
        );
        approvals::save(&approvals::approvals_path(&self.data_dir), &next)
            .map_err(|detail| (error_codes::EXT_007, detail))?;
        self.approvals = next;
        self.extensions[index].status = Status::Ready;
        shiori_log::info(
            "HUB",
            "拡張機能を承認した",
            &[("name", &manifest.name), ("version", &manifest.version), ("uses", &manifest.uses.join(","))],
        );
        Ok(())
    }
}

impl Extension {
    /// 別のプログラムの拡張機能なら、このOSの実行ファイルの場所。
    pub fn program_exe(&self) -> Option<PathBuf> {
        let m = self.manifest.as_ref().filter(|m| m.kind == Kind::Program)?;
        m.entry.for_this_os().map(|e| self.dir.join(e))
    }
}

/// 承認済みの別のプログラムの拡張機能を、全部起動する(詩織の起動時)。
pub fn start_ready_programs(hub: &Hub, programs: &process::Programs) {
    for ext in hub.extensions().iter().filter(|e| e.status == Status::Ready) {
        if let Some(exe) = ext.program_exe() {
            programs.start(&ext.folder, exe, ext.dir.clone());
        }
    }
}

/// ほかの拡張機能の`provides`にある操作を呼ぶ(詩織を通した呼び出し)。相手の説明書に無い操作は断る。
pub fn call_provided(
    hub_state: &HubState,
    programs: &process::Programs,
    target: &str,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, CallError> {
    let (kind, provided) = {
        let hub = hub_state.0.lock().map_err(|e| CallError::new(error_codes::EXT_009, e.to_string()))?;
        let ext = hub.ready(target).ok_or_else(|| CallError::new(error_codes::EXT_009, target))?;
        let m = ext.manifest.as_ref().ok_or_else(|| CallError::new(error_codes::EXT_009, target))?;
        (m.kind, m.provides.iter().any(|p| p.method == method))
    };
    if !provided {
        return Err(CallError::new(error_codes::EXT_020, format!("{target}の{method}")));
    }
    match kind {
        Kind::Program => programs.call(target, method, params, process::CALL_TIMEOUT),
        // 画面の部品の提供する操作を中継するのは、段階2以降(画面が開いていないと答えられないため)
        Kind::Ui => Err(CallError::new(error_codes::EXT_012, format!("画面の部品{target}の{method}"))),
    }
}

fn log_extension(ext: &Extension) {
    match (&ext.status, &ext.manifest) {
        (Status::Ready, Some(m)) => {
            shiori_log::info("HUB", "読み込んだ", &[("name", &m.name), ("version", &m.version), ("kind", &kind_label(m.kind))])
        }
        (Status::NeedsApproval { first_time, added }, Some(m)) => shiori_log::info(
            "HUB",
            if *first_time { "初めての拡張機能なので、承認を待つ" } else { "使うものが増えたので、承認を待つ" },
            &[("name", &m.name), ("version", &m.version), ("増えたもの", &added.join(","))],
        ),
        (Status::Invalid { code, detail }, _) => shiori_log::error(
            code.code,
            "説明書が読めないので、読み込まない",
            &[("フォルダ", &ext.folder), ("意味", &code.message), ("詳しく", detail)],
        ),
        _ => {}
    }
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Ui => "ui",
        Kind::Program => "program",
    }
}

// ---- 画面との窓口 ----

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UseDto {
    pub name: String,
    pub label: String,
}

fn use_dtos(names: &[String]) -> Vec<UseDto> {
    names
        .iter()
        .map(|n| UseDto { name: n.clone(), label: manifest::use_label(n).unwrap_or("").to_string() })
        .collect()
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionDto {
    pub folder: String,
    pub name: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub kind: Option<&'static str>,
    /// 画面の部品の入口(拡張機能のフォルダからの相対パス)。別のプログラムは`None`
    pub entry: Option<String>,
    pub places: Vec<&'static str>,
    pub uses: Vec<UseDto>,
    /// ほかの拡張機能から呼ばれてよい操作
    pub provides: Vec<String>,
    /// "ready" | "needs-approval" | "invalid"
    pub status: &'static str,
    pub first_time: bool,
    pub added_uses: Vec<UseDto>,
    pub error_code: Option<&'static str>,
    pub error_message: Option<String>,
    /// 別のプログラムの拡張機能の、いまの動き(画面の部品は`None`)
    pub runtime: Option<RunState>,
    pub restarts: u32,
}

impl From<&Extension> for ExtensionDto {
    fn from(e: &Extension) -> Self {
        let m = e.manifest.as_ref();
        let (status, first_time, added, error_code, error_message) = match &e.status {
            Status::Ready => ("ready", false, Vec::new(), None, None),
            Status::NeedsApproval { first_time, added } => ("needs-approval", *first_time, added.clone(), None, None),
            Status::Invalid { code, detail } => {
                ("invalid", false, Vec::new(), Some(code.code), Some(format!("{}: {detail}", code.message)))
            }
        };
        Self {
            folder: e.folder.clone(),
            name: m.map(|m| m.name.clone()),
            title: m.map(|m| m.title.clone()),
            version: m.map(|m| m.version.clone()),
            description: m.map(|m| m.description.clone()),
            kind: m.map(|m| kind_label(m.kind)),
            entry: m.filter(|m| m.kind == Kind::Ui).and_then(|m| m.entry.for_this_os()).map(str::to_string),
            places: m
                .map(|m| m.places.iter().map(|p| if *p == Place::Window { "window" } else { "pet" }).collect())
                .unwrap_or_default(),
            uses: m.map(|m| use_dtos(&m.uses)).unwrap_or_default(),
            provides: m.map(|m| m.provides.iter().map(|p| p.method.clone()).collect()).unwrap_or_default(),
            status,
            first_time,
            added_uses: use_dtos(&added),
            error_code,
            error_message,
            runtime: None,
            restarts: 0,
        }
    }
}

fn to_dtos(hub: &Hub, programs: &process::Programs) -> Vec<ExtensionDto> {
    hub.extensions()
        .iter()
        .map(|e| {
            let mut dto = ExtensionDto::from(e);
            if let Some((state, restarts)) = programs.state_of(&e.folder) {
                dto.runtime = Some(state);
                dto.restarts = restarts;
            }
            dto
        })
        .collect()
}

#[tauri::command]
pub fn list_extensions(
    state: tauri::State<'_, HubState>,
    programs: tauri::State<'_, ProgramsState>,
) -> Result<Vec<ExtensionDto>, String> {
    let hub = state.0.lock().map_err(|e| e.to_string())?;
    Ok(to_dtos(&hub, &programs.0))
}

#[tauri::command]
pub fn approve_extension(
    state: tauri::State<'_, HubState>,
    programs: tauri::State<'_, ProgramsState>,
    name: String,
) -> Result<Vec<ExtensionDto>, String> {
    let mut hub = state.0.lock().map_err(|e| e.to_string())?;
    hub.approve(&name).map_err(|(code, detail)| {
        shiori_log::error(code.code, "拡張機能を承認できなかった", &[("name", &name), ("詳しく", &detail)]);
        format!("{}: {} ({detail})", code.code, code.message)
    })?;
    // 別のプログラムなら、承認したときに起動する
    if let Some(ext) = hub.ready(&name) {
        if let Some(exe) = ext.program_exe() {
            programs.0.start(&name, exe, ext.dir.clone());
        }
    }
    Ok(to_dtos(&hub, &programs.0))
}

/// 本体の画面から、拡張機能の`provides`にある操作を呼ぶ(段階1の仮の入り口で、別のプログラムの
/// 動きを確かめるため)。時間がかかることがあるので、画面を止めないよう別のスレッドで待つ。
#[tauri::command]
pub async fn call_extension(
    app: tauri::AppHandle,
    name: String,
    method: String,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, CallError> {
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || {
        let hub = app.state::<HubState>();
        let programs = app.state::<ProgramsState>();
        let result = call_provided(&hub, &programs.0, &name, &method, &params.unwrap_or(serde_json::Value::Null));
        match &result {
            Ok(_) => shiori_log::info("HUB", "画面から拡張機能を呼んだ", &[("name", &name), ("method", &method)]),
            Err(e) => shiori_log::warn(e.code, "画面からの呼び出しが失敗した", &[("name", &name), ("method", &method), ("理由", &e.message)]),
        }
        result
    })
    .await
    .map_err(|e| CallError::new(error_codes::EXT_017, e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::manifest::tests::{make_ext, ui_manifest, TempDir};
    use super::*;

    #[test]
    fn 初めては承認待ち_承認すると使える_次の起動でも使える() {
        let t = TempDir::new("hub-approve");
        let exts = t.0.join("extensions");
        let data = t.0.join("data");
        make_ext(&exts, "hello-ui", &ui_manifest("hello-ui", r#""storage""#));
        let mut hub = Hub::load_from(exts.clone(), data.clone());
        assert_eq!(
            hub.extensions()[0].status,
            Status::NeedsApproval { first_time: true, added: vec!["storage".into()] }
        );
        assert!(hub.ready("hello-ui").is_none());
        hub.approve("hello-ui").unwrap();
        assert!(hub.ready("hello-ui").is_some());
        // 次の起動
        let hub = Hub::load_from(exts, data);
        assert!(hub.ready("hello-ui").is_some());
    }

    #[test]
    fn 使うものが増えたら聞き直す() {
        let t = TempDir::new("hub-added");
        let exts = t.0.join("extensions");
        let data = t.0.join("data");
        make_ext(&exts, "hello-ui", &ui_manifest("hello-ui", r#""storage""#));
        Hub::load_from(exts.clone(), data.clone()).approve("hello-ui").unwrap();
        make_ext(&exts, "hello-ui", &ui_manifest("hello-ui", r#""storage", "ext.call""#));
        let hub = Hub::load_from(exts, data);
        assert_eq!(
            hub.extensions()[0].status,
            Status::NeedsApproval { first_time: false, added: vec!["ext.call".into()] }
        );
    }

    #[test]
    fn 読めないものは1つだけ外し_作業用のフォルダは無視する() {
        let t = TempDir::new("hub-invalid");
        let exts = t.0.join("extensions");
        make_ext(&exts, "good", &ui_manifest("good", ""));
        make_ext(&exts, "bad", &ui_manifest("wrong-name", ""));
        std::fs::create_dir_all(exts.join("_work")).unwrap();
        let hub = Hub::load_from(exts, t.0.join("data"));
        let names: Vec<&str> = hub.extensions().iter().map(|e| e.folder.as_str()).collect();
        assert_eq!(names, vec!["bad", "good"]);
        assert!(matches!(&hub.extensions()[0].status, Status::Invalid { code, .. } if code.code == "EXT-003"));
        assert!(matches!(hub.extensions()[1].status, Status::NeedsApproval { .. }));
    }

    #[test]
    fn 壊れた承認の記録は脇へ移し_全部を承認待ちにする() {
        let t = TempDir::new("hub-broken");
        let exts = t.0.join("extensions");
        let data = t.0.join("data");
        make_ext(&exts, "hello-ui", &ui_manifest("hello-ui", ""));
        Hub::load_from(exts.clone(), data.clone()).approve("hello-ui").unwrap();
        std::fs::write(approvals::approvals_path(&data), "{ 壊れた").unwrap();
        let hub = Hub::load_from(exts, data.clone());
        assert!(hub.ready("hello-ui").is_none());
        let aside = std::fs::read_dir(&data).unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains(".broken-"));
        assert!(aside);
    }

    #[test]
    fn 無い名前は承認できない() {
        let t = TempDir::new("hub-missing");
        let mut hub = Hub::load_from(t.0.join("extensions"), t.0.join("data"));
        assert_eq!(hub.approve("nothing").unwrap_err().0.code, "EXT-008");
    }
}
