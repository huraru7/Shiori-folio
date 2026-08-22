//! 詩織Ver2.0(セカンドブレイン化)のMCPサーバー(設計指示書v3、9章)。
//!
//! Claude Code等のAIエージェントから、詩織のライブラリ(library/)を検索専用で
//! 参照するためのstdioベースMCPサーバー。書き込みツールは持たない(書き込みは
//! library/への直接ファイル操作+決定的ルーティングで行う、7章)。
//!
//! 会話UI(shiori-folioバイナリ)とは別プロセス・別バイナリ(このクレート内の
//! 追加[[bin]]ターゲット)として実装している。会話UIが起動していなくても、
//! embedding用llama-server・RAG Pythonサーバーを共有デーモンとして起動し
//! (shared_daemon::ensure_daemon_running、4章)、単体で検索が完結する。
//!
//! 起動: `cargo build --bin mcp_server`でビルドすると`target/{debug,release}/
//! mcp_server`が生成される。Claude Code側からはstdioで直接このバイナリを
//! 起動する設定を想定している。

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ServiceExt,
};

use shiori_folio_lib::{
    build_embedding_command, build_rag_command, library_root, load_search_backend_config, project_root,
    rag_client, shared_daemon,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchLibraryArgs {
    /// 検索クエリ(日本語可)
    query: String,
    /// 返すファイル数の上限(1回あたり)。スコア閾値による足切りは行わないため、
    /// 欲しい情報が見つからなければoffsetを増やして同じクエリで再検索すること
    #[serde(default = "default_limit")]
    limit: u32,
    /// ページングの開始位置。1回目は省略(0)、続きが欲しければ前回の
    /// offset+limitを指定して再検索する。返却件数がlimit未満ならそれ以上
    /// 候補が無いことを意味する
    #[serde(default)]
    offset: u32,
    /// frontmatterのauthor(human/claude-code)で絞り込む。省略可
    #[serde(default)]
    author: Option<String>,
    /// frontmatterのtypeで絞り込む。省略可
    #[serde(default)]
    r#type: Option<String>,
    /// frontmatterのprojectで絞り込む。省略可
    #[serde(default)]
    project: Option<String>,
}

fn default_limit() -> u32 {
    20
}

#[derive(Clone)]
struct ShioriLibrary {
    // #[tool_router]/#[tool_handler]マクロが展開するコード内部で参照される
    // (実機のJSON-RPC疎通テストでtools/list・tools/callとも正常動作を確認済み)。
    // rustcの未使用フィールド検出はマクロ展開後のコードまで追えないため警告が
    // 出るが、動作上は問題ない。
    #[allow(dead_code)]
    tool_router: ToolRouter<ShioriLibrary>,
}

#[tool_router]
impl ShioriLibrary {
    fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    // library/への物理パスの存在確認。設計指示書v3、2章「SSD未接続時の挙動」の
    // 通り、各ツールの最初のステップとして行い、無ければ即座にエラーを返す
    // (embeddingデーモンの起動処理などを試みる前に、ここで打ち切る)。
    fn ensure_library_present(&self) -> Result<(), McpError> {
        if library_root().is_dir() {
            Ok(())
        } else {
            Err(McpError::internal_error(
                "詩織のSSDが接続されていません(library/が見つかりません)",
                None,
            ))
        }
    }

    #[tool(
        description = "詩織のTier1プロファイル(_system/tier1-profile.md)を丸ごと返す。セッション開始時に検索を挟まず最初に呼ぶことを想定している"
    )]
    async fn get_context_profile(&self) -> Result<CallToolResult, McpError> {
        self.ensure_library_present()?;
        let path = library_root().join("_system/tier1-profile.md");
        let content = std::fs::read_to_string(&path).map_err(|e| {
            McpError::internal_error(format!("tier1-profile.mdの読み込みに失敗: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![ContentBlock::text(content)]))
    }

    #[tool(
        description = "詩織のライブラリ(library/)をクエリで検索し、ファイル単位に集約した見出し一覧を返す(本文は含まない。棚の場所を教えるだけで内容の合成はしない)。スコアによる足切りは行わないため、返却件数がlimit未満になるまでoffsetを増やして同じクエリで呼び直すと、関連しうる候補を漏らさず確認できる"
    )]
    async fn search_library(
        &self,
        Parameters(args): Parameters<SearchLibraryArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_library_present()?;

        let root = project_root();
        let backend =
            load_search_backend_config(&root).map_err(|e| McpError::internal_error(e, None))?;

        // embedding用llama-server・RAG Pythonサーバーの両方を共有デーモンとして
        // 起動確認する(設計指示書v3、4章)。会話UIが起動していなくてもここで
        // 単体完結する。戻り値のChildは(起動した場合)このプロセスの終了と共に
        // デタッチされる(shared_daemon.rs参照、意図した設計)。
        shared_daemon::ensure_daemon_running(
            &root,
            backend.embedding_port,
            ".shiori-embed.lock",
            30,
            || {
                build_embedding_command(&root, &backend.embedding_model_path, backend.embedding_port, true)
            },
        )
        .map_err(|e| McpError::internal_error(e, None))?;

        shared_daemon::ensure_daemon_running(&root, backend.rag_port, ".shiori-rag.lock", 60, || {
            build_rag_command(&root, backend.rag_port, true)
        })
        .map_err(|e| McpError::internal_error(e, None))?;

        let filter = if args.author.is_some() || args.r#type.is_some() || args.project.is_some() {
            Some(rag_client::LibrarySearchFilter {
                author: args.author.as_deref(),
                kind: args.r#type.as_deref(),
                project: args.project.as_deref(),
            })
        } else {
            None
        };

        let results =
            rag_client::search_library(backend.rag_port, &args.query, args.limit, args.offset, filter)
                .map_err(|e| McpError::internal_error(e, None))?;

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| McpError::internal_error(format!("結果のJSON化に失敗: {e}"), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for ShioriLibrary {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "詩織のライブラリ(セカンドブレイン)を検索するためのMCPサーバーです。\
                 セッション開始時にまずget_context_profileを呼び、その後は必要に応じて\
                 search_libraryで検索してください。書き込みはこのMCPサーバー経由ではなく\
                 library/への直接ファイル操作(frontmatter付きmd)で行います。保存時の\
                 配置ルールはlibrary/_system/CLAUDE.mdを参照してください。"
                    .to_string(),
            )
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stdioはMCPプロトコル通信専用のため、ログを出す場合は必ずstderrへ
    // (今回は最小構成のためログ自体は出していない)。
    let service = ShioriLibrary::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
