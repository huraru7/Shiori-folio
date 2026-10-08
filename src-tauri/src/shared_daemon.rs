//! 詩織Ver2.0(セカンドブレイン化)向けの共有デーモン起動ロジック。
//!
//! embedding用llama-server(nomic-embed-text)とRAG Pythonサーバー(FastAPI)は、
//! 会話UI・MCPサーバー・保存CLI等、複数のプロセスから共有される「先に呼んだ側が
//! 起動する」デーモンという位置づけになった(設計指示書v3、4章)。会話UIが
//! 起動していなくてもMCPサーバー単体で検索が完結する必要があるため。
//!
//! 手順はヘルスチェック→ロック取得→起動→ロック解放。ロックは複数プロセスが
//! 同時に「起動していないので自分が起動しよう」と判断して二重起動するのを防ぐ
//! ためのものであり、起動完了後は解放する(常時ロックし続けるものではない)。
//!
//! ここで起動したプロセスは、呼び出し元(会話UI・MCPサーバーいずれであっても)の
//! ライフタイムに縛られない。`std::process::Child`はdropしてもプロセスは
//! kill されない(Rust標準ライブラリの仕様)ため、意図的にハンドルを手放して
//! デタッチする。次回以降は別のどのプロセスからでもヘルスチェックで生存確認
//! するだけになる。

use fs4::fs_std::FileExt;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::Duration;

use crate::wait_for_health;

fn lock_path(root: &Path, lock_file_name: &str) -> PathBuf {
    root.join("data").join(lock_file_name)
}

/// ロックファイルの中身を起動したプロセスのPIDで上書きする。呼び出し元は
/// ロックを保持している状態(排他制御下)で呼ぶこと。
fn write_pid(lock_file: &mut std::fs::File, pid: u32) {
    let _ = lock_file.set_len(0);
    let _ = lock_file.seek(SeekFrom::Start(0));
    let _ = lock_file.write_all(pid.to_string().as_bytes());
    let _ = lock_file.flush();
}

/// ロックファイルに記録されたPIDを読む。ファイルが無い/中身が数値でない
/// 場合はNone(まだ一度も起動されていない、またはPID記録前の旧ロックファイル)。
pub fn read_daemon_pid(root: &Path, lock_file_name: &str) -> Option<u32> {
    let path = lock_path(root, lock_file_name);
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// ロックファイルに記録されたPIDが実際に生存していればそのPIDを返す。
/// 稼働判定(get_system_info)・kill対象特定(restart_llm_services/
/// retry_rag_service)の双方で、プロセスハンドルの保持有無ではなくこちらを
/// 正とする(2026-08-14、Ver2.0 Phase 2フォローアップ)。`sys`は呼び出し元が
/// 用意した(refresh_processes済みの)sysinfo::Systemを渡すこと(呼び出しの
/// たびに新規作成すると全プロセス列挙のコストがかかるため)。
pub fn daemon_pid_if_alive(sys: &sysinfo::System, root: &Path, lock_file_name: &str) -> Option<u32> {
    let pid = read_daemon_pid(root, lock_file_name)?;
    sys.process(sysinfo::Pid::from_u32(pid))?;
    Some(pid)
}

/// ロックファイルに記録されたPIDのプロセスをkillする。呼び出し元の
/// BackendStateがハンドルを保持していない(=外部プロセスが起動した)
/// デーモンを止めるための手段。生存していない、または記録が無い場合は
/// 何もせずfalseを返す。
pub fn kill_daemon(sys: &sysinfo::System, root: &Path, lock_file_name: &str) -> bool {
    let Some(pid) = daemon_pid_if_alive(sys, root, lock_file_name) else {
        return false;
    };
    match sys.process(sysinfo::Pid::from_u32(pid)) {
        Some(process) => process.kill(),
        None => false,
    }
}

/// portで`/health`が既に応答するなら何もしない。応答しなければロックファイルを
/// 取得してから`spawn`でプロセスを起動し、ヘルスチェックが通るまで待つ。
/// ロックが取得できない場合は他プロセスが起動処理中と判断し、少し待って
/// ヘルスチェックからやり直す(設計指示書v3、4章の手順そのまま)。
///
/// `startup_attempts`はヘルスチェックの最大試行回数(1回あたり最大2秒+
/// 待機0.5秒)。RAG Pythonサーバーはembedding用llama-serverより起動に時間が
/// かかりうるため、呼び出し元で余裕を持った値を渡すこと。
/// 戻り値`Ok(Some(child))`は「自分が今回起動した」ことを示す。呼び出し元が
/// 十分に長生きするプロセス(会話UI等)であれば、このハンドルを保持して
/// 従来通り再起動・終了時のkillに使ってよい。`Ok(None)`は「既に(自分以外の
/// 誰かが起動して)動いていた」ことを示し、その場合は呼び出し元がkillする
/// 手段を持たない(意図的な設計。詳細はモジュール冒頭のコメント参照)。
/// 呼び出し元がハンドルを使わずdropした場合、そのプロセスはkillされずに
/// 動き続ける(Rust標準ライブラリの`Child`はdropでは終了しない)。
pub fn ensure_daemon_running(
    root: &Path,
    port: u16,
    lock_file_name: &str,
    startup_attempts: u32,
    spawn: impl FnOnce() -> std::io::Result<Child>,
) -> Result<Option<Child>, String> {
    ensure_daemon_running_with_progress(root, port, lock_file_name, startup_attempts, spawn, None)
}

/// 起動処理の途中経過を書くファイル(RAGの`.index_progress.json`)の見方。
pub struct DaemonProgress<'a> {
    pub file: PathBuf,
    /// 進み具合のラベルが変わるたびに呼ばれる(起動画面への表示用)。
    pub on_label: &'a dyn Fn(&str),
}

/// 進捗ファイルが、この秒数より長く更新されていなければ「止まった」と見なす。
/// 1ファイルの埋め込み(大きな素材など)に数十秒かかっても止まったと誤認しない長さにしている。
const PROGRESS_STALE_SECS: u64 = 180;

/// 進捗ファイルが更新され続けていれば、その内容を表示用のラベルにして返す。
/// ファイルが無い・古い場合はNone(=作業中ではない)。
fn read_progress_label(file: &Path) -> Option<String> {
    let age = std::fs::metadata(file).ok()?.modified().ok()?.elapsed().ok()?;
    if age.as_secs() > PROGRESS_STALE_SECS {
        return None;
    }
    // 書き込み途中で読んだ場合などは中身が読めないが、更新はされているので作業中として扱う。
    let value: serde_json::Value = std::fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);
    let (Some(done), Some(total)) = (value["done"].as_u64(), value["total"].as_u64()) else {
        return Some("検索の索引を準備しています".to_string());
    };
    let n = (done + 1).min(total);
    Some(match value["phase"].as_str() {
        Some("rebuilding") => {
            format!("検索の索引を作り直しています({n}/{total})。数分かかることがあります")
        }
        _ => format!("検索の索引を更新しています({n}/{total})"),
    })
}

/// `wait_for_health`の進捗つき版。ヘルスチェックが通らなくても、進捗ファイルが
/// 更新され続けている間は「起動に失敗した」と数えず待ち続ける(RAGは起動時の同期が
/// 終わるまでポートを開かないため、全件の再登録が長引くと、進捗が無ければ失敗と
/// 誤判定されて子プロセスをkillしてしまう)。進捗が無い状態が`attempts`回続けば
/// 従来どおり失敗とする。
fn wait_for_health_with_progress(port: u16, attempts: u32, progress: Option<&DaemonProgress>) -> bool {
    let mut idle = 0;
    let mut last_label = String::new();
    while idle < attempts {
        if wait_for_health(port, 1) {
            return true;
        }
        match progress.and_then(|p| read_progress_label(&p.file).map(|label| (p, label))) {
            Some((p, label)) => {
                if label != last_label {
                    (p.on_label)(&label);
                    last_label = label;
                }
                idle = 0;
            }
            None => idle += 1,
        }
    }
    false
}

/// `ensure_daemon_running`の進捗つき版。`progress`を渡すと、起動待ちの間に
/// 進捗ファイルを見て、作業中ならkillせずに待ち、ラベルを通知する。
pub fn ensure_daemon_running_with_progress(
    root: &Path,
    port: u16,
    lock_file_name: &str,
    startup_attempts: u32,
    spawn: impl FnOnce() -> std::io::Result<Child>,
    progress: Option<&DaemonProgress>,
) -> Result<Option<Child>, String> {
    if wait_for_health(port, 1) {
        return Ok(None);
    }

    let path = lock_path(root, lock_file_name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("dataディレクトリの作成に失敗: {e}"))?;
    }
    let mut lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&path)
        .map_err(|e| format!("ロックファイル({lock_file_name})を開けません: {e}"))?;

    loop {
        match lock_file.try_lock_exclusive() {
            Ok(true) => {
                // ロック取得の直前に他プロセスが起動を終えている可能性があるため
                // 起動を試みる前にもう一度だけ確認する。
                if wait_for_health(port, 1) {
                    let _ = FileExt::unlock(&lock_file);
                    return Ok(None);
                }
                let outcome = match spawn() {
                    Ok(mut child) => {
                        if wait_for_health_with_progress(port, startup_attempts, progress) {
                            write_pid(&mut lock_file, child.id());
                            Ok(Some(child))
                        } else {
                            let _ = child.kill();
                            Err(format!(
                                "{lock_file_name}: 起動を試みましたがヘルスチェックがタイムアウトしました"
                            ))
                        }
                    }
                    Err(e) => Err(format!("{lock_file_name}: 起動コマンドの実行に失敗: {e}")),
                };
                let _ = FileExt::unlock(&lock_file);
                return outcome;
            }
            Ok(false) | Err(_) => {
                // 他プロセスが起動処理中(Ok(false))、またはロック取得自体に
                // 失敗した場合。少し待ってヘルスチェックからやり直す。
                std::thread::sleep(Duration::from_millis(500));
                if wait_for_health(port, 1) {
                    return Ok(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("shiori-progress-{}-{name}", std::process::id()))
    }

    #[test]
    fn progress_label_is_none_when_file_is_missing() {
        assert_eq!(read_progress_label(&temp_file("missing.json")), None);
    }

    #[test]
    fn progress_label_shows_rebuilding_and_syncing() {
        let file = temp_file("labels.json");
        std::fs::write(&file, r#"{"phase":"rebuilding","done":11,"total":87}"#).unwrap();
        let label = read_progress_label(&file).unwrap();
        assert!(label.contains("作り直して") && label.contains("12/87"), "{label}");

        std::fs::write(&file, r#"{"phase":"syncing","done":0,"total":2}"#).unwrap();
        let label = read_progress_label(&file).unwrap();
        assert!(label.contains("更新して") && label.contains("1/2"), "{label}");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn progress_label_treats_unreadable_but_fresh_file_as_working() {
        let file = temp_file("partial.json");
        std::fs::write(&file, "{\"phase\":").unwrap();
        assert_eq!(read_progress_label(&file).as_deref(), Some("検索の索引を準備しています"));
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn wait_gives_up_after_attempts_when_there_is_no_progress() {
        // 何も待ち受けていないポートで、進捗ファイルも無ければ、従来どおり失敗する。
        let on_label = |_: &str| {};
        let progress = DaemonProgress { file: temp_file("none.json"), on_label: &on_label };
        assert!(!wait_for_health_with_progress(1, 2, Some(&progress)));
    }
}
