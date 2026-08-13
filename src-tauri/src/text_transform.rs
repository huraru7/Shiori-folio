//! 汎用テキスト変換エンジン。
//!
//! クエリ正規化(検索前処理)と人称補正(LLM出力の後処理)は、どちらも
//! 「ルールに従って文字列を置換する」という同じ形をしているため、
//! 1つの薄いエンジンに統合している。ルールの中身はコードに埋め込まず
//! prompts/transforms/ 配下のJSONファイルに置き、prompts.rsの
//! load_system_prompt()と同じ方針で、呼び出しのたびにディスクから
//! 読み直す(ルールを直すだけなら再ビルド不要)。
//!
//! ルールは単純な文字列置換(順番に適用)のみをサポートする。位置条件
//! (文末のみ等)が必要なパターンが出てきた場合は、その時点でスキーマの
//! 拡張を検討する。

use serde::Deserialize;
use std::path::PathBuf;

use crate::project_root;

#[derive(Deserialize, Clone)]
pub struct Rule {
    pub id: String,
    pub pattern: String,
    pub replacement: String,
}

#[derive(Deserialize)]
struct RuleFile {
    rules: Vec<Rule>,
}

fn transforms_dir() -> PathBuf {
    project_root().join("prompts").join("transforms")
}

pub fn load_rules(file_name: &str) -> Result<Vec<Rule>, String> {
    let path = transforms_dir().join(file_name);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("変換ルールの読み込みに失敗({}): {e}", path.display()))?;
    let parsed: RuleFile = serde_json::from_str(&text)
        .map_err(|e| format!("変換ルールの解析に失敗({}): {e}", path.display()))?;
    Ok(parsed.rules)
}

/// ルールを順番に適用する。戻り値は(変換後のテキスト, 発動したルールidの一覧)。
/// idの一覧は呼び出し元がログに残す際に使う。
pub fn apply_rules(text: &str, rules: &[Rule]) -> (String, Vec<String>) {
    let mut result = text.to_string();
    let mut triggered = Vec::new();
    for rule in rules {
        if result.contains(&rule.pattern) {
            result = result.replace(&rule.pattern, &rule.replacement);
            triggered.push(rule.id.clone());
        }
    }
    (result, triggered)
}
