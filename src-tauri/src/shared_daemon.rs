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
                        if wait_for_health(port, startup_attempts) {
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
