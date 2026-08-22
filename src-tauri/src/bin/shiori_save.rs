//! 詩織Ver2.0(セカンドブレイン化)の保存CLI(設計指示書v3、7章)。
//!
//! Claude Codeはlibrary/へ直接ファイル操作で書き込めるが、配置先フォルダの
//! 判定・タグ/プロジェクトの正規化はLLMの裁量に任せず、このCLIが決定的な
//! ロジックとして担う(library/_system/CLAUDE.mdの配置ルールをそのまま実装)。
//!
//! 使用法: `shiori-save <mdファイルのパス>`
//! 対象ファイルにはfrontmatter(title/type/tags/project/author)が付与済みで
//! あることを前提とする。実行後、対象ファイルはlibrary/配下の決定先へ
//! 移動される(コピーではなく移動、元の場所には残らない)。frontmatterが
//! 不備な場合も保存自体は諦めず、00-inbox/へreasonフィールド付きで置く。

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use shiori_folio_lib::{library_root, load_search_backend_config, project_root, rag_client};

const VALID_TYPES: &[&str] = &[
    "project-log",
    "task",
    "decision",
    "principle",
    "insight",
    "experience",
    "glossary",
    "reference",
];

#[derive(Debug, Default, Deserialize, Serialize)]
struct Frontmatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    // title/type/tags/project/author/reason以外のfrontmatterフィールドは
    // 検証・ルーティングの対象外だが、保存時に消さず維持する。
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
struct ProjectsFile {
    #[serde(default)]
    projects: Vec<ProjectEntry>,
}

#[derive(Debug, Deserialize)]
struct ProjectEntry {
    id: String,
    #[serde(default)]
    domain: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TagsFile {
    #[serde(default)]
    tags: Vec<TagEntry>,
}

#[derive(Debug, Deserialize)]
struct TagEntry {
    canonical: String,
    #[serde(default)]
    aliases: Vec<String>,
}

fn main() -> Result<()> {
    let source = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("使用法: shiori-save <mdファイルのパス>"))?;
    let source = PathBuf::from(source);
    if !source.is_file() {
        bail!("ファイルが見つかりません: {}", source.display());
    }

    let root = library_root();
    if !root.is_dir() {
        bail!("詩織のSSDが接続されていません(library/が見つかりません)");
    }
    let system_dir = root.join("_system");
    let projects_yaml = system_dir.join("projects.yaml");
    let tags_yaml = system_dir.join("tags.yaml");

    let content = std::fs::read_to_string(&source)
        .with_context(|| format!("{}の読み込みに失敗", source.display()))?;
    let (mut fm, body) = split_frontmatter(&content)?;

    // タグは常に正規化する(inbox行きになる場合でも、表記ゆれの統一自体は
    // 後で見返したときに有用なため)。
    let mut newly_pending_tags = Vec::new();
    fm.tags = fm
        .tags
        .iter()
        .map(|raw| normalize_tag(&tags_yaml, raw, &mut newly_pending_tags))
        .collect::<Result<Vec<_>>>()?;

    let (dest_dir, inbox_reason) = match validate(&fm) {
        Some(reason) => {
            fm.reason = Some(reason.clone());
            (root.join("00-inbox"), Some(reason))
        }
        None => (decide_destination(&root, &fm, &projects_yaml)?, None),
    };

    std::fs::create_dir_all(&dest_dir)
        .with_context(|| format!("{}の作成に失敗", dest_dir.display()))?;
    let dest_path = unique_destination(&dest_dir, &source)?;

    let frontmatter_yaml = serde_yaml::to_string(&fm).context("frontmatterのYAML化に失敗")?;
    let new_content = format!("---\n{frontmatter_yaml}---\n{body}");
    std::fs::write(&dest_path, new_content)
        .with_context(|| format!("{}への書き込みに失敗", dest_path.display()))?;
    std::fs::remove_file(&source)
        .with_context(|| format!("移動元{}の削除に失敗", source.display()))?;

    // 書き込み時フック(詩織Ver3.0、データ管理法見直し2-5節)。RAGサーバーが
    // すでに起動していれば即座にこのファイルだけre-indexし、検索の都度の
    // 全件スキャンを待たず保存直後から検索対象にする。未起動時はここで
    // 起動を試みない(起動待ちでCLIの応答を遅らせないため)。次回のRAGサーバー
    // 起動時に行われる全件mtimeスキャンが安全網として拾う。
    if let Ok(relative) = dest_path.strip_prefix(&root) {
        let relative_str = relative.to_string_lossy();
        match load_search_backend_config(&project_root()) {
            Ok(backend) if rag_client::is_running(backend.rag_port) => {
                if let Err(e) = rag_client::reindex_file(backend.rag_port, &relative_str) {
                    eprintln!(
                        "警告: 即時re-indexに失敗しました({e})。次回のRAGサーバー起動時に反映されます。"
                    );
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("警告: 検索バックエンド設定の読み込みに失敗しました({e})。"),
        }
    }

    let result = serde_json::json!({
        "destination": dest_path.display().to_string(),
        "inbox_reason": inbox_reason,
        "newly_pending_tags": newly_pending_tags,
    });
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

// frontmatterの必須項目を検証する。不備があれば理由文字列を返す
// (Noneなら正常、呼び出し側はdecide_destinationへ進む)。
fn validate(fm: &Frontmatter) -> Option<String> {
    if fm.title.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Some("titleが空です".to_string());
    }
    if fm.tags.is_empty() {
        return Some("tagsが空です".to_string());
    }
    match fm.kind.as_deref() {
        Some(k) if VALID_TYPES.contains(&k) => None,
        other => Some(format!("typeが不正です(値: {other:?}, 許可値: {VALID_TYPES:?})")),
    }
}

// library/_system/CLAUDE.mdの「配置先の決め方」1〜4を実装する
// (5の日次ログ/セッション記録はfrontmatterのtype enumに対応する値が
// 定義されていないため、このCLIのスコープ外。GUI側の既存メモ保存機能
// との役割分担は別途要確認)。
fn decide_destination(root: &Path, fm: &Frontmatter, projects_yaml: &Path) -> Result<PathBuf> {
    let project = fm.project.as_deref().map(str::trim).filter(|p| !p.is_empty());

    match fm.kind.as_deref().expect("validateを通過済みのためSome") {
        "decision" => Ok(root.join("40-decisions")),

        "project-log" | "task" => match project {
            Some(p) => {
                ensure_project_registered(projects_yaml, p)?;
                Ok(root.join("20-projects").join(p))
            }
            None => Ok(root.join("00-inbox")),
        },

        "principle" | "insight" | "experience" | "glossary" => {
            let domain = match project {
                Some(p) => ensure_project_registered(projects_yaml, p)?,
                None => None,
            };
            Ok(root.join("30-knowledge").join(domain.unwrap_or_else(|| "general".to_string())))
        }

        "reference" => match project {
            Some(p @ ("profile" | "garden" | "portfolio")) => Ok(root.join("50-reference").join(p)),
            Some(p) => {
                ensure_project_registered(projects_yaml, p)?;
                Ok(root.join("50-reference").join(p))
            }
            // projectが空のreferenceは配置先を機械的に決められないため、
            // 他の不備ケースと同様inboxへ退避する。
            None => Ok(root.join("00-inbox")),
        },

        _ => unreachable!("validateで許可値のみ通過しているはず"),
    }
}

// frontmatterから取り出したYAMLのスカラー値を、コロンや日本語を含む文字列でも
// 壊れないよう安全にクォートしてシリアライズする。
fn yaml_scalar(s: &str) -> Result<String> {
    Ok(serde_yaml::to_string(s)?.trim_end().to_string())
}

// projects.yamlに指定idが存在するかを調べ、存在すればdomainを返す
// (domain未設定ならNone)。存在しなければ末尾にstatus: pendingで追記登録する。
// 既存ファイルの先頭コメント等を保つため、パース→丸ごと再シリアライズは
// せず、追記のみで済ませる。
fn ensure_project_registered(projects_yaml: &Path, id: &str) -> Result<Option<String>> {
    let text = std::fs::read_to_string(projects_yaml)
        .with_context(|| format!("{}の読み込みに失敗", projects_yaml.display()))?;
    let parsed: ProjectsFile = serde_yaml::from_str(&text)
        .with_context(|| format!("{}の解析に失敗", projects_yaml.display()))?;

    if let Some(entry) = parsed.projects.iter().find(|p| p.id == id) {
        return Ok(entry.domain.clone());
    }

    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(projects_yaml)
        .with_context(|| format!("{}への追記オープンに失敗", projects_yaml.display()))?;
    writeln!(
        f,
        "  - id: {}\n    domain: null\n    status: pending",
        yaml_scalar(id)?
    )?;
    Ok(None)
}

// tags.yamlのcanonical/aliasesと照合し、一致すればcanonical表記を返す。
// 一致しなければ末尾にstatus: pendingで自己登録し、そのままの表記を返す。
fn normalize_tag(tags_yaml: &Path, raw: &str, newly_pending: &mut Vec<String>) -> Result<String> {
    let text = std::fs::read_to_string(tags_yaml)
        .with_context(|| format!("{}の読み込みに失敗", tags_yaml.display()))?;
    let parsed: TagsFile = serde_yaml::from_str(&text)
        .with_context(|| format!("{}の解析に失敗", tags_yaml.display()))?;

    for entry in &parsed.tags {
        if entry.canonical == raw || entry.aliases.iter().any(|a| a == raw) {
            return Ok(entry.canonical.clone());
        }
    }
    // 同一実行内で同じ新規タグが複数回出てきても二重登録しない。
    if newly_pending.contains(&raw.to_string()) {
        return Ok(raw.to_string());
    }

    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(tags_yaml)
        .with_context(|| format!("{}への追記オープンに失敗", tags_yaml.display()))?;
    writeln!(
        f,
        "  - canonical: {}\n    aliases: []\n    status: pending",
        yaml_scalar(raw)?
    )?;
    newly_pending.push(raw.to_string());
    Ok(raw.to_string())
}

// 先頭`---`〜次の`---`をfrontmatterとしてパースし、(frontmatter, 本文)を返す。
fn split_frontmatter(content: &str) -> Result<(Frontmatter, String)> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let rest = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))
        .ok_or_else(|| anyhow!("frontmatter(先頭の---)が見つかりません"))?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| anyhow!("frontmatterの終端(---)が見つかりません"))?;
    let yaml_part = &rest[..end];
    let body = rest[end + "\n---".len()..]
        .strip_prefix("\r\n")
        .or_else(|| rest[end + "\n---".len()..].strip_prefix('\n'))
        .unwrap_or(&rest[end + "\n---".len()..]);

    let fm: Frontmatter =
        serde_yaml::from_str(yaml_part).context("frontmatterのYAML解析に失敗")?;
    Ok((fm, body.to_string()))
}

// 保存先ディレクトリ内でファイル名の衝突を避ける
// (source-2.md、source-3.md...と連番を振る)。
fn unique_destination(dir: &Path, source: &Path) -> Result<PathBuf> {
    let file_name = source
        .file_name()
        .ok_or_else(|| anyhow!("ファイル名の取得に失敗"))?
        .to_string_lossy()
        .to_string();
    let stem = file_name
        .strip_suffix(".md")
        .ok_or_else(|| anyhow!("拡張子が.mdではありません: {file_name}"))?;

    let mut candidate = dir.join(&file_name);
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem}-{n}.md"));
        n += 1;
    }
    Ok(candidate)
}
