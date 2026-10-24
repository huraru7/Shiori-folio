//! 左の縦の列の並びと非表示の記録(詩織Ver6.0の段階2)。
//!
//! `portable/data/ui/sidebar.json` に `{"order": [...], "hidden": [...]}` で置く。
//! ふらるが書く設定(`config.json`)とは分け、画面の並びだけを持つ。
//! 壊れていたら、日時を付けた名前で脇に退け、既定の並びで起動する(承認の記録と同じ扱い)。
//! 並びに無い項目を足す・消えた項目を無視する、という突き合わせは画面の側で行う
//! (列の項目の一覧を知っているのは画面の側のため)。

use crate::error_codes;
use crate::shiori_log;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SIDEBAR_FILE: &str = "sidebar.json";

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SidebarLayout {
    /// 列の項目のidの並び(上から)
    #[serde(default)]
    pub order: Vec<String>,
    /// 隠している項目のid
    #[serde(default)]
    pub hidden: Vec<String>,
}

pub fn sidebar_path(root: &Path) -> PathBuf {
    root.join("data").join("ui").join(SIDEBAR_FILE)
}

/// 本体の画面のアイコンの差し替えの置き場所。`{項目のid}.png`を置くと、同梱の仮の絵の代わりに使う。
pub fn icons_dir(root: &Path) -> PathBuf {
    root.join("data").join("ui").join("icons")
}

/// 差し替えのアイコンの名前として使えるか(英小文字・数字・ハイフンと`.png`だけ)。
fn is_icon_file(file: &str) -> bool {
    file.strip_suffix(".png").is_some_and(crate::hub::manifest::is_valid_name)
}

/// 差し替えのアイコンを読む(専用のURLの仕組みから呼ぶ)。
pub fn icon_override(file: &str) -> Option<Vec<u8>> {
    if !is_icon_file(file) {
        return None;
    }
    std::fs::read(icons_dir(&crate::project_root()).join(file)).ok()
}

/// 差し替えのアイコンがある項目のid。画面の側は、ここにあるものだけを読みにいく。
fn list_icon_overrides_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut ids: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|f| is_icon_file(f))
        .map(|f| f.trim_end_matches(".png").to_string())
        .collect();
    ids.sort();
    ids
}

/// 記録を読む。無ければ既定(空)。壊れていたら脇に退けて既定を返し、ログに書く。
pub fn load(path: &Path) -> SidebarLayout {
    let Ok(text) = std::fs::read_to_string(path) else {
        return SidebarLayout::default();
    };
    match serde_json::from_str::<SidebarLayout>(&text) {
        Ok(layout) => layout,
        Err(e) => {
            let aside = set_aside_broken(path);
            crate::repair::fixed(
                error_codes::UI_001,
                &[
                    ("内容", &e),
                    ("退けた先", &aside.map(|p| p.display().to_string()).unwrap_or_else(|| "退けられなかった".into())),
                ],
            );
            SidebarLayout::default()
        }
    }
}

/// 記録を書く(一時ファイルに書いてから置き換える)。続けて押されたときに一時ファイルを取り合わないよう、
/// 1つずつ書く。
pub fn save(path: &Path, layout: &SidebarLayout) -> Result<(), String> {
    static SAVING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SAVING.lock().unwrap_or_else(|e| e.into_inner());
    let fail = |e: String| {
        shiori_log::error(error_codes::UI_002.code, error_codes::UI_002.message, &[("内容", &e)]);
        format!("{}: {e}", error_codes::UI_002.code)
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| fail(format!("{} ({e})", dir.display())))?;
    }
    let text = serde_json::to_string_pretty(layout).map_err(|e| fail(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| fail(format!("{} ({e})", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|e| fail(format!("{} ({e})", path.display())))
}

fn set_aside_broken(path: &Path) -> Option<PathBuf> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let aside = path.with_file_name(format!("{SIDEBAR_FILE}.broken-{stamp}"));
    std::fs::rename(path, &aside).ok().map(|_| aside)
}

#[tauri::command]
pub async fn get_sidebar_layout() -> Result<SidebarLayout, String> {
    tauri::async_runtime::spawn_blocking(|| load(&sidebar_path(&crate::project_root())))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_icon_overrides() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(|| list_icon_overrides_in(&icons_dir(&crate::project_root())))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_sidebar_layout(layout: SidebarLayout) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        save(&sidebar_path(&crate::project_root()), &layout)?;
        shiori_log::info(
            "UI",
            "列の並びを保存した",
            &[("並び", &layout.order.join(",")), ("隠す", &layout.hidden.join(","))],
        );
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::manifest::tests::TempDir;

    #[test]
    fn 無ければ既定の並び() {
        let t = TempDir::new("sidebar-none");
        assert_eq!(load(&sidebar_path(&t.0)), SidebarLayout::default());
    }

    #[test]
    fn 書いたものを読み直せる() {
        let t = TempDir::new("sidebar-save");
        let path = sidebar_path(&t.0);
        let layout = SidebarLayout { order: vec!["home".into(), "library".into()], hidden: vec!["claude".into()] };
        save(&path, &layout).unwrap();
        assert_eq!(load(&path), layout);
    }

    #[test]
    fn 続けて書いても取り合わない() {
        let t = TempDir::new("sidebar-race");
        let path = sidebar_path(&t.0);
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let layout = SidebarLayout { order: vec![format!("item-{i}")], hidden: Vec::new() };
                    save(&path, &layout)
                })
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), Ok(()));
        }
        assert_eq!(load(&path).order.len(), 1);
    }

    #[test]
    fn 差し替えのアイコンは決まった名前のPNGだけ() {
        let t = TempDir::new("sidebar-icons");
        let dir = icons_dir(&t.0);
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["home.png", "library-browse.png", "Bad.png", "x.gif", "..png"] {
            std::fs::write(dir.join(f), b"png").unwrap();
        }
        assert_eq!(list_icon_overrides_in(&dir), vec!["home", "library-browse"]);
        assert!(!is_icon_file("../home.png"));
    }

    #[test]
    fn 壊れていたら脇に退けて既定の並び() {
        let t = TempDir::new("sidebar-broken");
        let path = sidebar_path(&t.0);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ 壊れた").unwrap();
        assert_eq!(load(&path), SidebarLayout::default());
        assert!(!path.exists());
        let aside = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("sidebar.json.broken-"));
        assert!(aside);
    }
}
