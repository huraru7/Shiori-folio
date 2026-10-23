//! Pythonの文字列の扱いに合わせる小さな部品。Python版と同じチャンク・同じ値を作るために使う。

/// Pythonの `str.splitlines()` と同じ区切りで行に分ける(区切りの文字は含めない)。
/// `\n`・`\r\n`・`\r` のほか、`\v`・`\f`・`\x1c`〜`\x1e`・`\x85`・` `・` ` でも切る。
pub fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let is_break = matches!(
            c,
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
        );
        if !is_break {
            continue;
        }
        out.push(&s[start..i]);
        let mut end = i + c.len_utf8();
        if c == '\r'
            && let Some(&(j, '\n')) = chars.peek()
        {
            chars.next();
            end = j + 1;
        }
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Pythonの `str.isspace()` と同じ判定(`str.strip()` が剥がす文字)。
pub fn py_isspace(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\x1c'..='\x1f')
}

pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}

/// Pythonの `Path.read_text()` と同じく、改行を `\n` にそろえる(`\r\n` と `\r` を `\n` にする)。
pub fn universal_newlines(s: &str) -> String {
    if !s.contains('\r') {
        return s.to_string();
    }
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// ファイルをUTF-8として読み、改行をそろえる。UTF-8でなければエラー(Pythonの `UnicodeDecodeError` に当たる)。
pub fn read_text(path: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let s = String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // PythonのUTF-8(BOMなし)は、先頭のBOMを文字として残す。ここでもそのまま残す
    Ok(universal_newlines(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitlinesはpythonと同じ() {
        assert_eq!(py_splitlines("a\nb\r\nc\rd"), vec!["a", "b", "c", "d"]);
        assert_eq!(py_splitlines("a\n"), vec!["a"]);
        assert_eq!(py_splitlines("a\n\nb"), vec!["a", "", "b"]);
        assert_eq!(py_splitlines(""), Vec::<&str>::new());
        assert_eq!(py_splitlines("x\u{2028}y"), vec!["x", "y"]);
    }

    #[test]
    fn 改行をそろえる() {
        assert_eq!(universal_newlines("a\r\nb\rc"), "a\nb\nc");
    }
}
