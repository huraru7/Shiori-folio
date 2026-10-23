//! 抽出した文に、秘密情報らしき文字列が含まれていないかの検査(`services/rag/extraction/secrets.py` と同じ)。
//! 見つかったら、その素材は索引に入れない(誤検知で検索に出ない素材が出る代わりに、漏れを防ぐ)。

use regex::Regex;
use std::sync::LazyLock;

static PATTERNS: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("秘密鍵のヘッダー", r"-----BEGIN (?:[A-Z]+ )?PRIVATE KEY(?: BLOCK)?-----"),
        ("AWSアクセスキー", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
        ("OpenAI/Anthropic系のAPIキー", r"\bsk-[A-Za-z0-9_\-]{20,}"),
        ("GitHubトークン", r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}|\bgithub_pat_[A-Za-z0-9_]{30,}"),
        ("Slackトークン", r"\bxox[abprs]-[A-Za-z0-9\-]{10,}"),
        ("Googleアクセスキー", r"\bAIza[0-9A-Za-z_\-]{35}\b"),
        (
            "鍵・パスワードの代入",
            r#"(?i)\b(?:api[_-]?key|secret[_-]?key|client[_-]?secret|access[_-]?token|auth[_-]?token|passwd|password)\s*[:=]\s*["']?[A-Za-z0-9/+_\-]{16,}"#,
        ),
    ]
    .into_iter()
    .map(|(name, p)| (name, Regex::new(p).unwrap()))
    .collect()
});

/// 最初に見つかった秘密情報の種類(名前)を返す。無ければNone。一致した文字列そのものは返さない。
pub fn find_secret(text: &str) -> Option<&'static str> {
    PATTERNS.iter().find(|(_, re)| re.is_match(text)).map(|(name, _)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 見つける() {
        assert_eq!(find_secret("key: AKIAABCDEFGHIJKLMNOP"), Some("AWSアクセスキー"));
        assert_eq!(find_secret("PASSWORD = \"abcdefghijklmnop1234\""), Some("鍵・パスワードの代入"));
        assert_eq!(find_secret("ただの文章です"), None);
    }
}
