//! エラーコードの一覧(詩織Ver6.0の段階1)。
//!
//! 形は「部品名-3桁の番号」(例: `RAG-012`)。外部のAIが修復するときの手がかりにするため、
//! ログと画面に同じコードを出す。本体のコードは、ここに番号と意味を並べて持つ(コードと
//! 一覧が離れないようにする)。拡張機能のコードは、それぞれの説明書(`extension.json`)に書く。
//!
//! 部品名: APP(本体の全体)・LOG・CFG(設定)・HUB(ハブ)・EXT(拡張機能の読み込み・承認・窓口)・
//! LLM・EMB(埋め込み)・RAG・STT(音声認識)・TTS(読み上げ)・MCP・SAVE・EJECT・LIB(library)・SHELF(棚)。
//! いまの機能のエラーには、手を入れるときに順に番号を付ける(段階1では一斉には付けない)。

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorCode {
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code)
    }
}

macro_rules! codes {
    ($($name:ident = $code:literal, $message:literal;)*) => {
        $(pub const $name: ErrorCode = ErrorCode { code: $code, message: $message };)*
        /// 全部のコード(一覧の画面や、重なりのテストに使う)
        pub const ALL: &[ErrorCode] = &[$($name),*];
    };
}

codes! {
    // ログ(shiori_log.rsの中では、初期化の前なので文字列で直接出している)
    LOG_001 = "LOG-001", "ログのフォルダを作れない";
    LOG_002 = "LOG-002", "ログのファイルを開けない";
    // ハブ(拡張機能の置き場の全体)
    HUB_001 = "HUB-001", "拡張機能のフォルダを読めない";
    // 拡張機能の説明書と承認
    EXT_001 = "EXT-001", "説明書(extension.json)が無い";
    EXT_002 = "EXT-002", "説明書がJSONとして読めないか、知らない項目・足りない項目がある";
    EXT_003 = "EXT-003", "説明書のnameがフォルダの名前と違う";
    EXT_004 = "EXT-004", "説明書の項目の値が正しくない";
    EXT_005 = "EXT-005", "説明書が指すファイル(入口・アイコン)が無いか、フォルダの外を指している";
    EXT_006 = "EXT-006", "説明書のusesに、詩織が貸さないものが書かれている";
    EXT_007 = "EXT-007", "承認の記録を読めないか、書けない";
    EXT_008 = "EXT-008", "その名前の拡張機能が無い";
    // 拡張機能からの頼みごと(窓口)と、画面の部品の配り方
    EXT_009 = "EXT-009", "拡張機能が使える状態ではない(無いか、承認されていない)";
    EXT_010 = "EXT-010", "詩織が知らない頼みごと";
    EXT_011 = "EXT-011", "説明書のusesに書いていないものを頼んだ";
    EXT_012 = "EXT-012", "まだ使えない頼みごと(これから作る)";
    EXT_013 = "EXT-013", "拡張機能のファイルを返せない(無い、承認されていない、フォルダの外など)";
    EXT_014 = "EXT-014", "頼みごとの中身(params)が正しくない";
    // 別のプログラムの拡張機能
    EXT_015 = "EXT-015", "別のプログラムの拡張機能が止まったので、再起動する";
    EXT_016 = "EXT-016", "別のプログラムの拡張機能が何度も止まるので、再起動をやめた";
    EXT_017 = "EXT-017", "別のプログラムの拡張機能が動いていないか、時間内に返事をしない";
    EXT_018 = "EXT-018", "別のプログラムの拡張機能を起動できない";
    EXT_019 = "EXT-019", "別のプログラムの拡張機能から届いた行が、決まりの形のJSONではない";
    EXT_020 = "EXT-020", "相手の拡張機能の説明書のprovidesに無い操作は呼べない";
}

/// コードから意味を引く。
pub fn lookup(code: &str) -> Option<&'static ErrorCode> {
    ALL.iter().find(|c| c.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn コードは重ならず形がそろっている() {
        let mut seen = std::collections::HashSet::new();
        for c in ALL {
            assert!(seen.insert(c.code), "重なり: {}", c.code);
            let (part, num) = c.code.rsplit_once('-').expect("部品名-番号の形");
            assert!(part.chars().all(|ch| ch.is_ascii_uppercase()), "部品名: {}", c.code);
            assert!(num.len() == 3 && num.chars().all(|ch| ch.is_ascii_digit()), "番号: {}", c.code);
        }
        assert_eq!(lookup("LOG-001").map(|c| c.message), Some("ログのフォルダを作れない"));
    }
}
