//! 語句の一致による検索(文字bigramのBM25)。`services/rag/lexical.py` と同じ式。
//! 埋め込み検索が言い回しの違いで取りこぼしたチャンクを、候補に足すために使う。

use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

pub const K1: f64 = 1.5;
pub const B: f64 = 0.75;

/// NFCにそろえ、小文字にし、空白を除いてから、2文字ずつに切る。1文字だけならその1文字。
pub fn bigrams(text: &str) -> Vec<String> {
    let s: Vec<char> = text.nfc().collect::<String>().to_lowercase().chars().filter(|c| !is_py_regex_space(*c)).collect();
    if s.len() < 2 {
        return if s.is_empty() { Vec::new() } else { vec![s.iter().collect()] };
    }
    s.windows(2).map(|w| w.iter().collect()).collect()
}

/// Pythonの正規表現の `\s`(Unicodeの文字列)と同じ判定。
fn is_py_regex_space(c: char) -> bool {
    crate::text::py_isspace(c)
}

pub struct LexicalIndex {
    tf: Vec<HashMap<String, u32>>,
    len: Vec<u32>,
    avg: f64,
    idf: HashMap<String, f64>,
}

impl LexicalIndex {
    pub fn new(texts: &[String]) -> Self {
        let tf: Vec<HashMap<String, u32>> = texts
            .iter()
            .map(|t| {
                let mut m = HashMap::new();
                for g in bigrams(t) {
                    *m.entry(g).or_insert(0) += 1;
                }
                m
            })
            .collect();
        let len: Vec<u32> = tf.iter().map(|m| m.values().sum()).collect();
        let avg = if len.is_empty() { 0.0 } else { len.iter().map(|&l| l as f64).sum::<f64>() / len.len() as f64 };
        let mut df: HashMap<&str, u32> = HashMap::new();
        for m in &tf {
            for w in m.keys() {
                *df.entry(w.as_str()).or_insert(0) += 1;
            }
        }
        let n = tf.len() as f64;
        let idf = df
            .into_iter()
            .map(|(w, c)| (w.to_string(), (1.0 + (n - c as f64 + 0.5) / (c as f64 + 0.5)).ln()))
            .collect();
        Self { tf, len, avg, idf }
    }

    /// queryに語句が重なるものを、BM25の高い順にn件(添字で)返す。点数0は除く。
    /// `accept` はメタデータの絞り込み。同点はPython版と同じく、添字の大きい方を先にする。
    pub fn top(&self, query: &str, n: usize, accept: impl Fn(usize) -> bool) -> Vec<usize> {
        let qs = bigrams(query);
        let mut scored: Vec<(f64, usize)> = Vec::new();
        for (i, (tf, &ln)) in self.tf.iter().zip(&self.len).enumerate() {
            if !accept(i) {
                continue;
            }
            let mut s = 0.0;
            for w in &qs {
                if let Some(&f) = tf.get(w) {
                    let f = f as f64;
                    s += self.idf.get(w).copied().unwrap_or(0.0) * f * (K1 + 1.0)
                        / (f + K1 * (1.0 - B + B * ln as f64 / self.avg));
                }
            }
            if s > 0.0 {
                scored.push((s, i));
            }
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)));
        scored.into_iter().take(n).map(|(_, i)| i).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigramはpythonと同じ() {
        assert_eq!(bigrams("Ab c"), vec!["ab", "bc"]);
        assert_eq!(bigrams("詩"), vec!["詩"]);
        assert!(bigrams(" ").is_empty());
        // NFDの「が」もNFCにそろう
        assert_eq!(bigrams("か\u{3099}く"), vec!["がく"]);
    }

    #[test]
    fn 重なりの多いものが上() {
        let texts: Vec<String> = ["起動が遅い件の対策", "検索の改善", "起動の並列化で起動が速くなった"].map(String::from).to_vec();
        let idx = LexicalIndex::new(&texts);
        let top = idx.top("起動が遅い", 10, |_| true);
        assert_eq!(top[0], 0);
        assert!(!top.contains(&1));
        assert!(idx.top("起動", 10, |i| i != 2).iter().all(|&i| i != 2));
    }
}
