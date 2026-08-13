//! SQLite(data/shiori.db)の接続と初期化。仕様書7章のスキーマに対応する。

use std::path::Path;

use rusqlite::Connection;

pub fn open(root: &Path) -> Result<Connection, String> {
    let db_path = root.join("data").join("shiori.db");
    let conn = Connection::open(&db_path).map_err(|e| format!("SQLite接続に失敗: {e}"))?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS conversations (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          mode TEXT NOT NULL,
          role TEXT NOT NULL,
          content TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tts_failures (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          error TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| format!("テーブル作成に失敗: {e}"))?;
    Ok(conn)
}
