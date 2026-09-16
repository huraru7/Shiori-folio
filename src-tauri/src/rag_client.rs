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

#[derive(Serialize)]
struct ReindexFileRequest<'a> {
    path: &'a str,
}

// 書き込み時フック(詩織Ver3.0、データ管理法見直し2-5節)向け。shiori-save CLI
// 等がlibrary/への保存直後に呼び、検索の都度の全件スキャンを待たず対象
// ファイル1件だけを即座に埋め込む(services/rag/app.pyの/reindex_file)。
pub fn reindex_file(port: u16, relative_path: &str) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/reindex_file");
    let body = ReindexFileRequest { path: relative_path };

    ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .send_json(&body)
        .map_err(|e| format!("即時re-indexに失敗: {e}"))?;
    Ok(())
}

// RAGサーバーが応答するかを短いタイムアウトで確認する。shiori-save CLIのような
// 書き込み系CLIが、未起動のRAGサーバーの起動待ちで応答を止めてしまわないよう、
// 起動を試みる前にこれで疎通確認する用途(未起動なら即時re-indexを諦め、次回の
// RAGサーバー起動時の全件スキャンに任せる)。
pub fn is_running(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/health");
    ureq::get(&url).timeout(Duration::from_millis(500)).call().is_ok()
}

#[derive(Serialize, Default)]
pub struct LibrarySearchFilter<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub kind: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<&'a str>,
}

#[derive(Serialize)]
struct SearchLibraryRequest<'a> {
    query: &'a str,
    limit: u32,
    offset: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<LibrarySearchFilter<'a>>,
}

#[derive(Deserialize, Serialize)]
pub struct FileHeading {
    pub heading: String,
    pub rerank_score: f64,
}

#[derive(Deserialize, Serialize)]
pub struct SearchLibraryResultItem {
    pub source: String,
    pub source_category: String,
    pub headings: Vec<FileHeading>,
    pub best_score: f64,
    // 実ファイルへの絶対パス(見つからなければ空文字列)。app.py側で
    // source_category配下をrglobして解決している(2026-09-16追加)。
    pub path: String,
}

// MCPサーバーのsearch_libraryツール向け(詩織Ver2.0設計指示書v3、9章)。
// services/rag/app.pyの/search_libraryを呼ぶ。/searchと異なりファイル単位に
// 集約された結果(本文を含まない)が返る。
//
// 【Ver3.0で変更】スコア閾値による足切りを廃止し、offset+limitのページング
// 方式に変更した(データ管理法見直し3-3節)。呼び出し側は1回目offset=0で呼び、
// 欲しい情報が見つからなければoffsetを増やして(20, 40...)再クエリする。
pub fn search_library(
    port: u16,
    query: &str,
    limit: u32,
    offset: u32,
    filter: Option<LibrarySearchFilter>,
) -> Result<Vec<SearchLibraryResultItem>, String> {
    let url = format!("http://127.0.0.1:{port}/search_library");
    let body = SearchLibraryRequest { query, limit, offset, filter };

    ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .send_json(&body)
        .map_err(|e| format!("図書館検索に失敗: {e}"))?
        .into_json()
        .map_err(|e| format!("図書館検索応答の解析に失敗: {e}"))
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

#[derive(Deserialize)]
pub struct LibraryFileHeading {
    pub heading: String,
}

#[derive(Deserialize)]
pub struct LibraryFileItem {
    pub source: String,
    pub source_category: String,
    pub headings: Vec<LibraryFileHeading>,
}

// スタンドアロン図書館UI(Phase 7、1冊=1ファイルの表示単位への変更)向け。
// list_all()と違い、ファイル(source)単位に集約された結果(本文を含まない)が
// 返る(services/rag/app.pyの/list_all_library)。
pub fn list_all_library(port: u16) -> Result<Vec<LibraryFileItem>, String> {
    let url = format!("http://127.0.0.1:{port}/list_all_library");

    ureq::get(&url)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| format!("蔵書一覧の取得に失敗: {e}"))?
        .into_json()
        .map_err(|e| format!("蔵書一覧の応答解析に失敗: {e}"))
}
