//! メタデータの絞り込み。Python版がChromaDBに渡していた `where` の形のうち、
//! 使っているもの(完全一致・`$ne`・`$in`・`$nin`・`$and`)だけを持つ。

use serde_json::{Map, Value};

pub type Meta = Map<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub enum Where {
    Eq(String, String),
    Ne(String, String),
    In(String, Vec<String>),
    Nin(String, Vec<String>),
    And(Vec<Where>),
}

impl Where {
    pub fn eq(k: &str, v: &str) -> Self {
        Where::Eq(k.into(), v.into())
    }

    /// 条件を `$and` でつなぐ(1つならそのまま、無ければNone)。
    pub fn all(mut conds: Vec<Where>) -> Option<Where> {
        match conds.len() {
            0 => None,
            1 => conds.pop(),
            _ => Some(Where::And(conds)),
        }
    }

    pub fn matches(&self, meta: &Meta) -> bool {
        let get = |k: &str| meta.get(k).and_then(Value::as_str);
        match self {
            Where::Eq(k, v) => get(k) == Some(v.as_str()),
            // 項目が無いメタデータは「違う」として通す(Python版の語句の一致の判定と同じ)
            Where::Ne(k, v) => get(k) != Some(v.as_str()),
            Where::In(k, vs) => get(k).is_some_and(|x| vs.iter().any(|v| v == x)),
            Where::Nin(k, vs) => get(k).is_none_or(|x| vs.iter().all(|v| v != x)),
            Where::And(ws) => ws.iter().all(|w| w.matches(meta)),
        }
    }
}

pub fn matches(meta: &Meta, w: Option<&Where>) -> bool {
    w.is_none_or(|w| w.matches(meta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(v: Value) -> Meta {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn 絞り込み() {
        let m = meta(json!({"status": "new", "source_category": "20-areas", "project": "shiori"}));
        assert!(Where::Ne("status".into(), "deprecated".into()).matches(&m));
        assert!(!Where::Nin("status".into(), vec!["outdated".into(), "new".into()]).matches(&m));
        assert!(Where::In("source_category".into(), vec!["20-areas".into()]).matches(&m));
        let w = Where::all(vec![Where::eq("project", "shiori"), Where::eq("kind", "memory")]).unwrap();
        assert!(!w.matches(&m));
        assert!(matches(&m, None));
    }
}
