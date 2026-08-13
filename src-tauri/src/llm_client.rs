//! LLM(llama-server、OpenAI互換 /v1/chat/completions)への問い合わせ。

use std::time::Duration;

use serde_json::Value;

// Function Calling用。messagesはロール履歴(system/user/assistant/tool)を含むJSON配列、
// toolsはツール定義のJSON配列。生のレスポンスJSONをそのまま返し、呼び出し側で
// choices[0].messageのcontent/tool_callsを取り出す(型を固定しすぎず柔軟に扱うため)。
//
// 補足: OpenAI互換のtool_choiceで特定関数呼び出しを強制する方式も試したが、
// system-prompt.mdのような長い system メッセージと組み合わせると、この
// llama-serverはtool_choiceを無視してfinish_reason=stopの通常応答を返す
// ことが実測で確認された(短いsystemメッセージでは機能する)。そのため
// アイデンティティ質問の安全網はtool_choiceに頼らず、Rust側で直接
// search_knowledgeを実行してcontextとして注入する方式に切り替えている
// (lib.rsのbuild_identity_guard_context参照)。
// enable_thinking: false はQwen3系の思考モード(reasoning_content)を止めるための
// パラメータ(2026-08-13、Qwen3-8B移行検証時に追加)。Qwen2.5等、思考モード自体を
// 持たないモデルのJinjaテンプレートはこのキーワード引数を単に参照しないだけなので
// 無害(--jinja起動が前提。エラーにはならないことをQwen2.5-7Bで実機確認済み)。
// 思考モードが有効なままだと、短い雑談でも応答に数秒〜20秒台の遅延が生じることが
// 実測で分かっている(voice_integration_checkの最大応答時間が22.79s→5.96sに短縮)。
pub fn chat_with_tools(port: u16, messages: &Value, tools: &Value) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let body = serde_json::json!({
        "messages": messages,
        "tools": tools,
        "chat_template_kwargs": { "enable_thinking": false }
    });

    ureq::post(&url)
        .timeout(Duration::from_secs(60))
        .send_json(&body)
        .map_err(|e| format!("LLM呼び出しに失敗: {e}"))?
        .into_json()
        .map_err(|e| format!("LLM応答の解析に失敗: {e}"))
}
