//! 拡張機能の承認の記録(詩織Ver6.0の段階1)。
//!
//! `portable/data/extensions/_approvals.json` に、名前・版・承認した`uses`・日時を残す。
//! 説明書の`uses`が承認した範囲の中なら、版が上がっても聞き直さない。範囲の外のものが
//! 増えたら、もう一度承認の画面を出す。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const APPROVALS_FILE: &str = "_approvals.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub version: String,
    pub uses: Vec<String>,
    pub approved_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ApprovalFile {
    /// 記録の形の版(形を変えたときに見分けるため)
    #[serde(default = "current_format")]
    pub format: u32,
    #[serde(default)]
    pub extensions: BTreeMap<String, Approval>,
}

fn current_format() -> u32 {
    1
}

pub fn approvals_path(data_dir: &Path) -> PathBuf {
    data_dir.join(APPROVALS_FILE)
}

/// 記録を読む。ファイルが無ければ空の記録。壊れていればエラー(呼び出し側で、空として扱い知らせる)。
pub fn load(path: &Path) -> Result<ApprovalFile, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{} ({e})", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ApprovalFile { format: 1, ..Default::default() }),
        Err(e) => Err(format!("{} ({e})", path.display())),
    }
}

/// 記録を書く。途中で落ちても壊れないよう、別のファイルに書いてから置き換える。
pub fn save(path: &Path, file: &ApprovalFile) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} ({e})", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{} ({e})", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{} ({e})", path.display()))
}

/// 壊れた記録を、日時を付けた名前に移して残す(上書きで消さないため)。
pub fn set_aside_broken(path: &Path) -> Option<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let aside = path.with_file_name(format!("{APPROVALS_FILE}.broken-{stamp}"));
    std::fs::rename(path, &aside).ok().map(|_| aside)
}

/// 承認していない使うもの。`None`は、まだ1度も承認していない(記録が無い)。
pub fn unapproved_uses(uses: &[String], approval: Option<&Approval>) -> Option<Vec<String>> {
    let approval = approval?;
    Some(uses.iter().filter(|u| !approval.uses.contains(u)).cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::manifest::tests::TempDir;

    fn approval(uses: &[&str]) -> Approval {
        Approval { version: "0.1.0".into(), uses: uses.iter().map(|s| s.to_string()).collect(), approved_at: "t".into() }
    }

    #[test]
    fn 承認していない使うもの() {
        let uses = vec!["storage".to_string(), "ext.call".to_string()];
        assert_eq!(unapproved_uses(&uses, None), None);
        assert_eq!(unapproved_uses(&uses, Some(&approval(&["storage", "ext.call"]))), Some(vec![]));
        // 減ったときは聞き直さない
        assert_eq!(unapproved_uses(&uses[..1], Some(&approval(&["storage", "ext.call"]))), Some(vec![]));
        // 増えたものだけを返す
        assert_eq!(unapproved_uses(&uses, Some(&approval(&["storage"]))), Some(vec!["ext.call".to_string()]));
    }

    #[test]
    fn 書いて読み戻せる_無ければ空_壊れていればエラー() {
        let t = TempDir::new("approvals");
        let path = approvals_path(&t.0.join("data"));
        assert_eq!(load(&path).unwrap().extensions.len(), 0);
        let mut file = ApprovalFile { format: 1, ..Default::default() };
        file.extensions.insert("hello-ui".into(), approval(&["storage"]));
        save(&path, &file).unwrap();
        assert_eq!(load(&path).unwrap(), file);
        std::fs::write(&path, "{ 壊れた").unwrap();
        assert!(load(&path).is_err());
        let aside = set_aside_broken(&path).unwrap();
        assert!(aside.is_file());
        assert!(!path.exists());
    }
}
