//! RAG検索サーバー(services/rag/app.py、/search)への問い合わせ。

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct SearchRequest<'a> {
    query: &'a str,
    top_k: u32,
}

#[derive(Deserialize)]
pub struct SearchResultItem {
    // RAGAS評価用にPython側で追加されたchunk ID。KnowledgeResultDto経由でUIにも渡す。
    pub id: String,
    pub text: String,
    pub source: String,
    pub heading: String,
    pub distance: f64,
    // リランカー(Python側/search、2026-08-07追加)のスコア。Rust側では現状使わない。
    #[allow(dead_code)]
    pub rerank_score: f64,
    pub source_category: String,
}

pub fn search(port: u16, query: &str, top_k: u32) -> Result<Vec<SearchResultItem>, String> {
    let url = format!("http://127.0.0.1:{port}/search");
    let body = SearchRequest { query, top_k };

    ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .send_json(&body)
        .map_err(|e| format!("RAG検索に失敗: {e}"))?
        .into_json()
        .map_err(|e| format!("RAG応答の解析に失敗: {e}"))
}

#[derive(Serialize)]
struct AddDocumentRequest<'a> {
    id: &'a str,
    text: &'a str,
    heading: &'a str,
    source: &'a str,
    source_category: &'a str,
}

// メモ機能(memo_guard)向け。ingest.pyの全件再投入を待たず、単一ドキュメントを
// その場で埋め込み・ChromaDBへ追加する(services/rag/app.pyの/add_document)。
pub fn add_document(
    port: u16,
    id: &str,
    text: &str,
    heading: &str,
    source: &str,
    source_category: &str,
) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/add_document");
    let body = AddDocumentRequest { id, text, heading, source, source_category };

    ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .send_json(&body)
        .map_err(|e| format!("メモの即時登録に失敗: {e}"))?;
    Ok(())
}

#[derive(Deserialize)]
pub struct ChunkItem {
    pub id: String,
    pub text: String,
    pub source: String,
    pub heading: String,
    pub source_category: String,
}

// スタンドアロン図書館UI(2026-08-12、図書館ビジョン統合仕様書3-2)向け。
// ChromaDBの全チャンクを検索なしで一括取得する(services/rag/app.pyの/list_all)。
pub fn list_all(port: u16) -> Result<Vec<ChunkItem>, String> {
    let url = format!("http://127.0.0.1:{port}/list_all");

    ureq::get(&url)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| format!("蔵書一覧の取得に失敗: {e}"))?
        .into_json()
        .map_err(|e| format!("蔵書一覧の応答解析に失敗: {e}"))
}
