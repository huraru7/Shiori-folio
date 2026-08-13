//! 詩織のシステムプロンプト・ツール定義の読み込み。
//!
//! 「発話パターンに応じてツールを呼ぶ道具」ではなく、「いつもそこに居て、
//! 会話の流れの中で自然と関連する記憶を差し出してくる存在」として詩織を
//! 設計する。詳しい人物設定はprompts/system-prompt.md本文に統合済み
//! (旧prompts/shiori-character.mdは「作っても読み込まれない資料」に
//! なっていたため、2026-08-07に本文へ統合し削除した)。
//!
//! 自発的な想起は、LLMに「検索すべきか」を判断させるFunction Calling方式では
//! 確率的なブレが大きすぎたため、判断ステップ自体を無くした「常時軽量検索」
//! (send_message内のbuild_passive_recall_context)に置き換えた。search_knowledge
//! ツールは、明示的な検索依頼に応える経路として残る。
//!
//! プロンプト本文・ツール定義はコードに埋め込まず prompts/ 配下の外部ファイルに
//! 置き、文言調整だけであればビルドし直さずに反映できるようにしている。

use std::path::PathBuf;

use crate::project_root;

const TOOL_NAMES: [&str; 1] = ["search_knowledge"];

fn prompts_dir() -> PathBuf {
    project_root().join("prompts")
}

pub fn load_system_prompt() -> Result<String, String> {
    let path = prompts_dir().join("system-prompt.md");
    std::fs::read_to_string(&path)
        .map_err(|e| format!("システムプロンプトの読み込みに失敗({}): {e}", path.display()))
}

pub fn load_tool_definitions() -> Result<serde_json::Value, String> {
    let mut tools = Vec::with_capacity(TOOL_NAMES.len());
    for name in TOOL_NAMES {
        let path = prompts_dir().join("tools").join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("ツール定義の読み込みに失敗({}): {e}", path.display()))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("ツール定義の解析に失敗({}): {e}", path.display()))?;
        tools.push(value);
    }
    Ok(serde_json::Value::Array(tools))
}
