//! 埋め込みのサーバー(llama-server --embedding、既定は8082)への問い合わせ。
//! `services/rag/embedding_client.py` と同じく、nomic-embed-textの決まりの前置きを付ける。

use std::time::Duration;

use serde::Deserialize;

pub struct Embedder {
    url: String,
    agent: ureq::Agent,
}

#[derive(Deserialize)]
struct Resp {
    data: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    embedding: Vec<f32>,
}

impl Embedder {
    pub fn new(port: u16) -> Self {
        Self {
            url: format!("http://127.0.0.1:{port}/v1/embeddings"),
            agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build(),
        }
    }

    /// 質問は `search_query: `、文書は `search_document: ` を前に付けて埋め込む。
    pub fn embed(&self, text: &str, is_query: bool) -> Result<Vec<f32>, String> {
        let prefix = if is_query { "search_query: " } else { "search_document: " };
        let resp: Resp = self
            .agent
            .post(&self.url)
            .send_json(serde_json::json!({ "input": format!("{prefix}{text}") }))
            .map_err(|e| format!("埋め込みのサーバーに問い合わせられない: {e}"))?
            .into_json()
            .map_err(|e| format!("埋め込みの応答を読めない: {e}"))?;
        resp.data.into_iter().next().map(|i| i.embedding).ok_or_else(|| "埋め込みの応答が空".to_string())
    }
}
