//! 検索用の本文(パッセージ)の組み立て(`services/rag/passage.py` と同じ)。
//! 埋め込み・リランク・語句の一致の3か所で同じ形にそろえる。

use crate::chunking::NO_HEADING;

pub fn passage_text(title: &str, summary: &str, heading: &str, text: &str) -> String {
    let head: Vec<&str> = [title, summary].into_iter().filter(|x| !x.is_empty()).collect();
    let head = head.join("。");
    let body = if heading.is_empty() || heading == NO_HEADING { text.to_string() } else { format!("{heading}: {text}") };
    if head.is_empty() { body } else { format!("{head}\n{body}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 形() {
        assert_eq!(passage_text("T", "S", "背景", "本文"), "T。S\n背景: 本文");
        assert_eq!(passage_text("", "", NO_HEADING, "本文"), "本文");
        assert_eq!(passage_text("T", "", "", "本文"), "T\n本文");
    }
}
