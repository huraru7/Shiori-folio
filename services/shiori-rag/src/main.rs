//! 詩織のRAG(検索・索引・素材の抽出)。いまの `services/rag`(Python)と同じHTTPの窓口を話す、
//! Rustの別のプログラム(詩織Ver6.0の段階5)。
//!
//! 使い方:
//!   shiori-rag serve [--root <portableのフォルダ>] [--port <番号>] [--index-dir <索引の置き場>]
//!                    [--model <リランカーのモデルのフォルダ>] [--log-dir <ログの置き場>]
//!                    [--library <libraryのフォルダ>] [--shelf <棚のフォルダ>] [--workers <並列の数>]
//!   shiori-rag extract <素材> <結果のJSON>     (サーバーが内部で使う、抽出の別プロセス)
//!   shiori-rag dump-chunks <libraryのフォルダ> (チャンク分けを、埋め込まずにJSONで書き出す。比較用)
//!
//! 既定は、実行ファイルから上へたどって `config.json` のあるフォルダを `--root` とし、
//! ポートと埋め込みのポートは `config.json`(`rag.port`・`embedding.port`)から読む。
//! 索引は `<root>/data/rag-index/`、モデルは `<root>/models/rerank/`、ログは `<root>/data/logs/`。

mod chunking;
mod dates;
mod embed;
mod errors;
mod extract;
mod filter;
mod frontmatter;
mod indexing;
mod lexical;
mod log;
mod passage;
mod rerank;
mod search;
mod server;
mod store;
mod text;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

fn main() {
    // candleの中の行列の掛け算は1スレッドにする(候補ごとに並列にするので、取り合うと遅い)。
    // 何かのスレッドを作る前に設定する
    // SAFETY: まだほかのスレッドが無いうちに、1回だけ設定する
    unsafe { std::env::set_var("RAYON_NUM_THREADS", "1") };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("extract") if args.len() == 3 => extract::worker_main(Path::new(&args[1]), Path::new(&args[2])),
        Some("dump-chunks") if args.len() == 2 => dump_chunks(Path::new(&args[1])),
        Some("serve") => match serve(&args[1..]) {
            Ok(()) => 0,
            Err(e) => {
                log::error("RAG", "起動できない", &[("理由", &e)]);
                1
            }
        },
        _ => {
            eprintln!("使い方: shiori-rag serve [--root DIR] [--port N] … / shiori-rag extract <素材> <結果> / shiori-rag dump-chunks <DIR>");
            2
        }
    };
    std::process::exit(code);
}

fn options(args: &[String]) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let key = k.strip_prefix("--").ok_or_else(|| format!("知らない引数: {k}"))?;
        let v = it.next().ok_or_else(|| format!("{k} の値が無い"))?;
        map.insert(key.to_string(), v.clone());
    }
    Ok(map)
}

/// 実行ファイルから上へたどって、`config.json` のあるフォルダ(本体の `project_root()` と同じ)。
fn find_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    loop {
        if dir.join("config.json").is_file() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// `library/`(や `knowledge/`)の場所。ポータブル版は root の直下、開発のツリーは親の直下(Python版と同じ)。
fn data_dir(root: &Path, name: &str) -> PathBuf {
    let portable = root.join(name);
    if portable.is_dir() { portable } else { root.parent().unwrap_or(root).join(name) }
}

fn serve(args: &[String]) -> Result<(), String> {
    let opts = options(args)?;
    let root = match opts.get("root") {
        Some(r) => PathBuf::from(r),
        None => find_root().ok_or_else(|| format!("{}: config.jsonが見つからない", errors::RAG_001.code))?,
    };
    let config: Value = std::fs::read_to_string(root.join("config.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| format!("{}: {}", errors::RAG_001.code, root.join("config.json").display()))?;
    let log_settings: log::LogSettings =
        config.get("log").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let log_dir = opts.get("log-dir").map(PathBuf::from).unwrap_or_else(|| root.join("data").join("logs"));
    log::init(&log_dir, &log_settings);

    let port: u16 = match opts.get("port") {
        Some(p) => p.parse().map_err(|_| format!("ポートの番号が正しくない: {p}"))?,
        None => config.pointer("/rag/port").and_then(Value::as_u64).unwrap_or(8083) as u16,
    };
    let embed_port = config.pointer("/embedding/port").and_then(Value::as_u64).unwrap_or(8082) as u16;
    let index_dir = opts.get("index-dir").map(PathBuf::from).unwrap_or_else(|| root.join("data").join("rag-index"));
    let model_dir = opts.get("model").map(PathBuf::from).unwrap_or_else(|| root.join("models").join("rerank"));
    let library = opts.get("library").map(PathBuf::from).unwrap_or_else(|| data_dir(&root, "library"));
    let shelf = opts.get("shelf").map(PathBuf::from).unwrap_or_else(|| data_dir(&root, "knowledge"));
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    // 段階0の試作で速かった並列の数(20スレッドのWindowsで16、10コアのMacで8)に近づける
    let workers: usize = match opts.get("workers") {
        Some(w) => w.parse().map_err(|_| format!("並列の数が正しくない: {w}"))?,
        None => (cores - cores / 5).max(1),
    };
    log::info(
        "RAG",
        "起動した",
        &[
            ("版", &env!("CARGO_PKG_VERSION")),
            ("ポート", &port),
            ("library", &library.display()),
            ("索引", &index_dir.display()),
            ("並列", &workers),
        ],
    );

    let t = Instant::now();
    let reranker = rerank::Reranker::load(&model_dir, workers)
        .map_err(|e| format!("{}: {} ({e:#})", errors::RAG_007.code, model_dir.display()))?;
    log::info("RAG", "リランカーを読み込んだ", &[("秒", &format!("{:.2}", t.elapsed().as_secs_f64()))]);

    let open = |name: &'static str, dir: PathBuf, state: PathBuf| {
        indexing::Index::open(name, dir, state.clone())
            .map_err(|e| format!("{}: {} ({e})", errors::RAG_002.code, state.display()))
    };
    let lib_index = open("LIB", library.clone(), index_dir.clone())?;
    let shelf_index = open("SHELF", shelf.clone(), index_dir.join("shelf"))?;
    let embedder = embed::Embedder::new(embed_port);

    // 起動時の同期が終わってからポートを開く(Python版と同じ。待つ側は進み具合のファイルを見る)
    let t = Instant::now();
    let r = lib_index.sync(&embedder);
    log::info("LIB", "索引を同期した", &[("新規", &r.added.len()), ("変更", &r.updated.len()), ("削除", &r.removed.len()), ("秒", &format!("{:.1}", t.elapsed().as_secs_f64()))]);
    if shelf.is_dir() {
        let r = shelf_index.sync(&embedder);
        log::info("SHELF", "索引を同期した", &[("新規", &r.added.len()), ("変更", &r.updated.len()), ("削除", &r.removed.len())]);
    }
    let app = Arc::new(server::App { library: lib_index, shelf: shelf_index, embedder, reranker });
    server::serve(app, port, 4)
}

/// チャンク分けを埋め込まずに書き出す(Python版との比較用)。1行に1記事のJSON。
fn dump_chunks(dir: &Path) -> i32 {
    for path in indexing::list_indexable_files(dir) {
        let rel = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        let Ok(text) = text::read_text(&path) else { continue };
        let chunks: Vec<Value> = if frontmatter::is_indexable(&text) {
            chunking::chunk_markdown(&text)
                .into_iter()
                .map(|c| serde_json::json!({"heading": c.heading, "text": c.text}))
                .collect()
        } else {
            Vec::new()
        };
        let f = frontmatter::read(&text);
        let mut fields = serde_json::json!({
            "title": f.title, "summary": f.summary, "status": f.status, "related": f.related,
            "project": f.project, "type": f.type_, "author": f.author, "kind": f.kind, "date": f.recency_date,
        });
        if let Some((cited, learned, topic)) = f.shelf {
            fields["cited_from"] = cited.into();
            fields["learned"] = learned.into();
            fields["topic"] = topic.into();
        }
        println!("{}", serde_json::json!({"rel": rel, "fields": fields, "chunks": chunks}));
    }
    0
}
