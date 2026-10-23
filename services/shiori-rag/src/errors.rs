//! エラーコードの一覧(部品名 `RAG`)。形は本体の `src-tauri/src/error_codes.rs` と同じ
//! 「部品名-3桁の番号」。切り替えのときに、本体の一覧にも同じものを足す。

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
        #[allow(dead_code)]
        pub const ALL: &[ErrorCode] = &[$($name),*];
    };
}

codes! {
    RAG_001 = "RAG-001", "設定(config.json)を読めない";
    RAG_002 = "RAG-002", "索引のフォルダを作れないか、読めない";
    RAG_003 = "RAG-003", "索引のファイルが壊れている(その記事は作り直す)";
    RAG_004 = "RAG-004", "索引のファイルを書けない";
    RAG_005 = "RAG-005", "記事を読めない(文字コードなど)ので、索引に入れずに飛ばした";
    RAG_006 = "RAG-006", "埋め込みのサーバーに問い合わせられない";
    RAG_007 = "RAG-007", "リランカーのモデルを読み込めない";
    RAG_008 = "RAG-008", "リランカーの計算に失敗した";
    RAG_009 = "RAG-009", "素材に秘密情報らしき文字列があるので、抽出を止めた";
    RAG_010 = "RAG-010", "素材の抽出に失敗した";
    RAG_011 = "RAG-011", "ポートを開けない";
    RAG_012 = "RAG-012", "要求の形が正しくない";
    RAG_013 = "RAG-013", "要求の処理に失敗した";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn コードは重ならず形がそろっている() {
        let mut seen = std::collections::HashSet::new();
        for c in ALL {
            assert!(seen.insert(c.code), "重なり: {}", c.code);
            let (part, num) = c.code.rsplit_once('-').unwrap();
            assert_eq!(part, "RAG");
            assert!(num.len() == 3 && num.chars().all(|ch| ch.is_ascii_digit()));
        }
    }
}
