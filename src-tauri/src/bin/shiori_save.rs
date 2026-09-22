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

// 詩織Ver3.1(journal廃止・project/area配下へのkind統合、2026-09-16)。
// journalをtypeから廃止し、project/areaそれぞれの配下にjournal/resourceの
// 2種類(kindフィールド)を持たせる形に変更した。resourceは「project/area
// に紐づかない外部由来の知識」専用に純化し、ふらるさん自身についての記録は
// 新設のprofileへ独立させた。
const VALID_TYPES: &[&str] = &["project", "area", "resource", "profile"];
// type: project/areaの記事だけが持つ、記事の性質(作業ログか、そこから
// 生まれた意思決定・知見か)。type: resource/profileには存在しない
// (存在してもdecide_destinationでは参照しない)。
const VALID_KINDS: &[&str] = &["journal", "resource"];
// 詩織Ver3.2(40-profile/のサブフォルダ細分化、2026-09-21)。type: profileの
// 記事だけが持つ分類。未指定を許容する点がkindと異なり、profile-huraru.md
// のような「プロフィール全体の入り口」的な記事はcategoryなしのまま
// 40-profile/直下に置かれる。
const VALID_CATEGORIES: &[&str] = &[
    "temperament-and-thinking",
    "values",
    "life-history",
    "relationships",
    "self-image",
    "daily-and-work",
];

fn default_index() -> bool {
    true
}

fn default_status() -> String {
    "new".to_string()
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Frontmatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    // 詩織Ver3.1で新設。type: project/areaの記事のみ使う(journal | resource)。
    // YAML上のフィールド名は"kind"だが、上のkind(YAML上の"type")と紛らわしい
    // ためRust側の変数名はsub_kindとした。
    #[serde(rename = "kind", skip_serializing_if = "Option::is_none")]
    sub_kind: Option<String>,
    // 詩織Ver3.2で新設。type: profileの記事のみ使う(6分類の列挙、未指定可)。
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<String>,
    // 本文とは別の1〜2文の要約(検索結果表示・rerank精度向上に使う)。
    // 必須化はしない(無い場合は単に空欄のまま)。
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    // 検索対象に含めるか(services/rag/indexing.py側で参照する)。デフォルトtrue。
    #[serde(default = "default_index")]
    index: bool,
    // draft/new/outdated/deprecated/disputedの5値を想定するが、typeと違い
    // 判断を誤っても実害が小さいためCLI側でのenumバリデーションはしない。
    #[serde(default = "default_status")]
    status: String,
    // 内部的に関連する他記事へのリンク(将来のリンクグラフの元データ)。
    #[serde(default)]
    related: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    // 上記以外のfrontmatterフィールドは検証・ルーティングの対象外だが、
    // 保存時に消さず維持する。
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
        Some(k) if VALID_TYPES.contains(&k) => {
            // project/areaはkind(journal|resource)も必須。resource/profileには
            // 存在しない概念なので、指定されていても無視する(バリデーション
            // 対象外)。
            if k == "project" || k == "area" {
                match fm.sub_kind.as_deref() {
                    Some(sk) if VALID_KINDS.contains(&sk) => None,
                    other => Some(format!(
                        "kindが不正です(値: {other:?}, 許可値: {VALID_KINDS:?}。typeがproject/areaの記事には必須です)"
                    )),
                }
            } else if k == "profile" {
                // categoryは未指定を許容する(profile-huraru.md想定)。
                // 指定されている場合のみ値の妥当性を検証する。
                match fm.category.as_deref() {
                    None => None,
                    Some(c) if VALID_CATEGORIES.contains(&c) => None,
                    Some(c) => Some(format!(
                        "categoryが不正です(値: {c:?}, 許可値: {VALID_CATEGORIES:?})"
                    )),
                }
            } else {
                None
            }
        }
        other => Some(format!("typeが不正です(値: {other:?}, 許可値: {VALID_TYPES:?})")),
    }
}

// library/_system/CLAUDE.mdの「配置先の決め方」を実装する(詩織Ver3.0、
// データ管理法見直し2-2節)。typeの判定・付与自体はこのCLIの役目ではなく、
// 書き込みを行うAI(Claude Code)が自律的に行う前提(人間はtypeの値を直接
// 参照・指定しない)。このCLIは決定されたtypeを機械的にフォルダへ変換する
// だけの決定的ロジックを担う。
fn decide_destination(root: &Path, fm: &Frontmatter, projects_yaml: &Path) -> Result<PathBuf> {
    let project = fm.project.as_deref().map(str::trim).filter(|p| !p.is_empty());

    match fm.kind.as_deref().expect("validateを通過済みのためSome") {
        // 期限・ゴールのある進行中の取り組み。kind(journal|resource)で
        // さらにサブフォルダへ分ける(詩織Ver3.1、journal廃止統合)。
        "project" => match project {
            Some(p) => {
                ensure_project_registered(projects_yaml, p)?;
                let sub_kind = fm.sub_kind.as_deref().expect("validateを通過済みのためSome");
                Ok(root.join("10-projects").join(p).join(sub_kind))
            }
            None => Ok(root.join("00-inbox")),
        },

        // 終わりのない継続的関心領域。projectフィールドを識別子として流用する
        // (huraru.com運営・詩織開発等、type: projectと同じ命名空間で管理する)。
        // projectと同じくkindでjournal/resourceに分ける。
        "area" => match project {
            Some(p) => {
                ensure_project_registered(projects_yaml, p)?;
                let sub_kind = fm.sub_kind.as_deref().expect("validateを通過済みのためSome");
                Ok(root.join("20-areas").join(p).join(sub_kind))
            }
            None => Ok(root.join("00-inbox")),
        },

        // 【詩織Ver3.1で意味を変更】project/areaに紐づかない、外部から
        // 与えられた知識・参考資料専用に純化した(project/area内で生まれた
        // 意思決定・知見はkind: resourceとしてそれぞれの配下に置くようになった
        // ため)。projectフィールドが付いていても無視し、常に直下に置く。
        "resource" => Ok(root.join("30-resources")),

        // ふらるさん自身についての記録(詩織Ver3.1で新設。旧30-resources/profile/
        // の独立後継)。詩織Ver3.2でcategory(6分類)によるサブフォルダ分けに
        // 対応。category未指定の記事(profile-huraru.md等)は直下に置く。
        "profile" => match fm.category.as_deref() {
            Some(c) => Ok(root.join("40-profile").join(c)),
            None => Ok(root.join("40-profile")),
        },

        _ => unreachable!("validateで許可値のみ通過しているはず"),
    }
}

// frontmatterから取り出したYAMLのスカラー値を、コロンや日本語を含む文字列でも
// 壊れないよう安全にクォートしてシリアライズする。
fn yaml_scalar(s: &str) -> Result<String> {
    Ok(serde_yaml::to_string(s)?.trim_end().to_string())
}

// projects.yamlに指定idが存在するかを調べる。存在しなければ末尾に
// status: pendingで追記登録する。既存ファイルの先頭コメント等を保つため、
// パース→丸ごと再シリアライズはせず、追記のみで済ませる。
//
// 【Ver3.0で変更】旧30-knowledgeのdomain逆引きが不要になった(30-resourcesは
// project別サブフォルダ構成のため)ため、戻り値のdomainは廃止した。
fn ensure_project_registered(projects_yaml: &Path, id: &str) -> Result<()> {
    let text = std::fs::read_to_string(projects_yaml)
        .with_context(|| format!("{}の読み込みに失敗", projects_yaml.display()))?;
    let parsed: ProjectsFile = serde_yaml::from_str(&text)
        .with_context(|| format!("{}の解析に失敗", projects_yaml.display()))?;

    if parsed.projects.iter().any(|p| p.id == id) {
        return Ok(());
    }

    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(projects_yaml)
        .with_context(|| format!("{}への追記オープンに失敗", projects_yaml.display()))?;
    writeln!(f, "  - id: {}\n    status: pending", yaml_scalar(id)?)?;
    Ok(())
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
