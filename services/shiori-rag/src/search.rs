//! 候補の取り方とリランク(`services/rag/app.py` の `_retrieve_and_rerank` と同じ手順)。
//!
//! 1. 埋め込みで近い順に、コレクションの件数(上限200)まで取る
//! 2. 語句の一致(BM25)の上位100件のうち、まだ入っていないものを足す
//! 3. 全部をリランカーで点数づけする
//! 4. 現在モードは時間減衰を掛けて点数順(`/search` だけは0.3未満を落とす)。経緯モードは日付の新しい順

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::dates;
use crate::embed::Embedder;
use crate::filter::{Meta, Where, matches};
use crate::indexing::{Index, passage_of};
use crate::rerank::Reranker;

pub const RERANK_CANDIDATE_POOL_MAX: usize = 200;
pub const LEXICAL_CANDIDATES: usize = 100;
/// 語句の一致だけで見つかった候補の距離(実際には計算していない。距離で絞る用途には通らない大きな値)
pub const LEXICAL_ONLY_DISTANCE: f64 = 99.0;
pub const RERANK_SCORE_THRESHOLD: f64 = 0.3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Current,
    History,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    pub document: String,
    pub metadata: Meta,
    pub distance: f64,
    pub score: f64,
}

impl Candidate {
    pub fn meta_str(&self, key: &str) -> &str {
        self.metadata.get(key).and_then(Value::as_str).unwrap_or("")
    }
}

pub struct Options {
    pub filter: Option<Where>,
    pub apply_threshold: bool,
    pub mode: Mode,
    pub time_decay: bool,
    pub now: DateTime<Utc>,
}

impl Options {
    pub fn new(filter: Option<Where>, apply_threshold: bool, mode: Mode, time_decay: bool) -> Self {
        Self { filter, apply_threshold, mode, time_decay, now: Utc::now() }
    }
}

pub fn retrieve_and_rerank(
    index: &Index,
    embedder: &Embedder,
    reranker: &Reranker,
    query: &str,
    opts: Options,
) -> Result<Vec<Candidate>, String> {
    let query_embedding = embedder.embed(query, true)?;
    // 現在モードだけ、status: deprecated を候補の時点で外す(経緯モードは見比べたいので外さない)
    let combined = match opts.mode {
        Mode::Current => {
            let ne = Where::Ne("status".into(), "deprecated".into());
            Some(match opts.filter {
                Some(w) => Where::And(vec![w, ne]),
                None => ne,
            })
        }
        Mode::History => opts.filter,
    };

    let mut candidates: Vec<Candidate> = {
        let col = index.collection.read().unwrap();
        let pool = col.count().min(RERANK_CANDIDATE_POOL_MAX);
        col.query(&query_embedding, pool, combined.as_ref())
            .into_iter()
            .map(|(r, d)| Candidate { id: r.id.clone(), document: r.document.clone(), metadata: r.metadata.clone(), distance: d, score: 0.0 })
            .collect()
    };
    let lexical = index.lexical();
    let seen: std::collections::HashSet<String> = candidates.iter().map(|c| c.id.clone()).collect();
    for i in lexical.index.top(query, LEXICAL_CANDIDATES, |i| matches(&lexical.records[i].metadata, combined.as_ref())) {
        let r = &lexical.records[i];
        if seen.contains(&r.id) {
            continue;
        }
        candidates.push(Candidate {
            id: r.id.clone(),
            document: r.document.clone(),
            metadata: r.metadata.clone(),
            distance: LEXICAL_ONLY_DISTANCE,
            score: 0.0,
        });
    }

    let passages: Vec<String> = candidates
        .iter()
        .map(|c| {
            passage_of(&crate::store::Record {
                id: String::new(),
                document: c.document.clone(),
                metadata: c.metadata.clone(),
                embedding: Vec::new(),
            })
        })
        .collect();
    let scores = reranker.rerank(query, &passages).map_err(|e| format!("リランカーの計算に失敗: {e}"))?;
    for (c, s) in candidates.iter_mut().zip(scores) {
        c.score = s;
    }

    if opts.mode == Mode::History {
        // 新しい順。日付不明は最後(並べ替えは安定なので、同じ日付どうしは元の順のまま)
        candidates.sort_by(|a, b| {
            let da = dates::parse(a.meta_str("date"));
            let db = dates::parse(b.meta_str("date"));
            db.cmp(&da)
        });
        return Ok(candidates);
    }
    if opts.time_decay {
        for c in candidates.iter_mut() {
            c.score *= dates::time_decay_factor(c.meta_str("date"), opts.now);
        }
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    if opts.apply_threshold {
        candidates.retain(|c| c.score >= RERANK_SCORE_THRESHOLD);
    }
    Ok(candidates)
}

/// ファイル(source)ごとの集まり。並びは、候補の中での初出の順。
pub struct FileGroup {
    pub source: String,
    pub meta: Meta,
    pub headings: Vec<(String, f64)>,
    pub best_score: f64,
}

pub fn group_by_file(candidates: &[Candidate]) -> Vec<FileGroup> {
    let mut order: Vec<FileGroup> = Vec::new();
    let mut pos: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for c in candidates {
        let source = c.meta_str("source").to_string();
        let i = *pos.entry(source.clone()).or_insert_with(|| {
            order.push(FileGroup { source: source.clone(), meta: c.metadata.clone(), headings: Vec::new(), best_score: c.score });
            order.len() - 1
        });
        order[i].headings.push((c.meta_str("heading").to_string(), c.score));
    }
    order
}

/// クエリにプロジェクト名が含まれるとき、そのプロジェクトのmemoryを先頭へ移す(`memory_rank.py` と同じ)。
/// それ以外の並びは変えない。
pub fn promote_project_memory(groups: Vec<FileGroup>, query: &str) -> Vec<FileGroup> {
    let q = casefold(query);
    let is_promoted = |g: &FileGroup| {
        let s = |k: &str| g.meta.get(k).and_then(Value::as_str).unwrap_or("");
        s("kind") == "memory" && !s("project").is_empty() && q.contains(&casefold(s("project")))
    };
    let (mut promoted, rest): (Vec<_>, Vec<_>) = groups.into_iter().partition(is_promoted);
    promoted.extend(rest);
    promoted
}

/// Pythonの `str.casefold()` に近い比較用の形(小文字にし、ßをssにする)。
fn casefold(s: &str) -> String {
    s.to_lowercase().replace('ß', "ss")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cand(source: &str, heading: &str, score: f64, extra: Value) -> Candidate {
        let mut m = json!({"source": source, "heading": heading}).as_object().unwrap().clone();
        for (k, v) in extra.as_object().unwrap() {
            m.insert(k.clone(), v.clone());
        }
        Candidate { id: format!("{source}-{heading}"), document: String::new(), metadata: m, distance: 0.0, score }
    }

    #[test]
    fn ファイルごとに集めてmemoryを先頭へ() {
        let cs = vec![
            cand("a.md", "h1", 0.9, json!({})),
            cand("shiori/memory.md", "h", 0.5, json!({"kind": "memory", "project": "shiori"})),
            cand("a.md", "h2", 0.4, json!({})),
            cand("x/memory.md", "h", 0.3, json!({"kind": "memory", "project": "x"})),
        ];
        let g = group_by_file(&cs);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].headings, vec![("h1".into(), 0.9), ("h2".into(), 0.4)]);
        let p = promote_project_memory(g, "Shioriの状態");
        let order: Vec<&str> = p.iter().map(|g| g.source.as_str()).collect();
        assert_eq!(order, vec!["shiori/memory.md", "a.md", "x/memory.md"]);
    }
}
