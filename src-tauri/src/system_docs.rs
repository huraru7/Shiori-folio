//! 規約・記憶画面(詩織Ver4.1、memoryの閲覧はVer4.2)向け。詩織のシステムを決めている書類
//! (libraryの保存規約、タグ/プロジェクトの台帳、起動時のプロフィール、プロンプト、設定)と、
//! 共通記憶MD(全体用・各プロジェクトのmemory)を、読み取り専用で一覧・取得する。
//!
//! これらは索引の対象外(`library/_system/`はRAGが除外している)で、図書館の画面には
//! 出ない。画面から任意のパスを読めてしまわないよう、**読める書類はここで組み立てる
//! 許可リストだけ**にしている。呼び出し側が渡すのは書類のidだけで、idからパスを作らず、
//! 許可リストの中から一致するものを探す(`../`などを渡しても一致せず、読めない)。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

// 1ファイルの読み取り上限。規約・設定は数十KBなので、これを超えるものは想定外として読まない。
const MAX_BYTES: u64 = 1024 * 1024;

// 画面での並び順(グループの出現順がそのまま画面の順になる)。
const GROUP_MEMORY: &str = "共通記憶(memory)";
const GROUP_LIBRARY: &str = "libraryの規約";
const GROUP_LEDGER: &str = "台帳";
const GROUP_PROFILE: &str = "起動時のプロフィール";
const GROUP_BEHAVIOR: &str = "詩織の動作";

struct Entry {
    id: String,
    group: &'static str,
    label: String,
    path: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemDocDto {
    pub id: String,
    pub group: String,
    pub label: String,
    pub file_name: String,
    // 画面での表示方法。"markdown" | "yaml" | "json" | "text"。
    pub format: String,
    pub exists: bool,
    pub size: u64,
    pub mtime: f64,
}

fn format_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("md") => "markdown",
        Some("yaml") | Some("yml") => "yaml",
        Some("json") => "json",
        _ => "text",
    }
}

// フォルダ直下のjson(隠しファイルとmacOSの`._`は除く)を名前順で返す。
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|e| e.to_str()) == Some("json")
                && !p.file_name().and_then(|n| n.to_str()).unwrap_or(".").starts_with('.')
        })
        .collect();
    files.sort();
    files
}

fn file_name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

// memoryのfrontmatterにある`updated`(最終更新日時のISO8601文字列)。無い・読めないときは空文字。
// 同じ書式(+09:00)で書かれるので、文字列のまま新しい順に並べられる。
fn memory_updated(path: &Path) -> String {
    let Ok(meta) = std::fs::metadata(path) else {
        return String::new();
    };
    if meta.len() > MAX_BYTES {
        return String::new();
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return String::new();
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix("updated:") {
            return value.trim().trim_matches('"').to_string();
        }
    }
    String::new()
}

// `10-projects/{project}/memory.md`・`20-areas/{project}/memory.md`をフォルダから探し、
// updatedの新しい順で(project id, パス)を返す。新しいプロジェクトのmemoryは、コードを直さず
// 一覧に出る。idに使うproject idは、フォルダ名がそのまま入るので、`/`や`.`始まりは除く。
fn project_memories(library_root: &Path) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, String, PathBuf)> = Vec::new();
    for top in ["10-projects", "20-areas"] {
        let Ok(read) = std::fs::read_dir(library_root.join(top)) else {
            continue;
        };
        for dir in read.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()) {
            let name = file_name_of(&dir);
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            let memory = dir.join("memory.md");
            if memory.is_file() && !found.iter().any(|(n, _, _)| *n == name) {
                found.push((name, memory_updated(&memory), memory));
            }
        }
    }
    // updatedの新しい順。同じ・無い場合はproject idの名前順で安定させる。
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    found.into_iter().map(|(name, _, path)| (name, path)).collect()
}

fn entries(library_root: &Path, project_root: &Path) -> Vec<Entry> {
    let system = library_root.join("_system");
    let organizing = system.join("library-organizing");
    let prompts = project_root.join("prompts");
    let mut list = Vec::new();
    let mut add = |id: String, group: &'static str, label: String, path: PathBuf| {
        list.push(Entry { id, group, label, path });
    };

    add(
        "memory/global".into(),
        GROUP_MEMORY,
        "全体のmemory(memory-global.md)".into(),
        library_root.join("40-profile").join("memory-global.md"),
    );
    for (project, path) in project_memories(library_root) {
        add(format!("memory/project/{project}"), GROUP_MEMORY, format!("{project}のmemory"), path);
    }

    add("library/CLAUDE.md".into(), GROUP_LIBRARY, "保存の規約(CLAUDE.md)".into(), system.join("CLAUDE.md"));
    for (file, label) in [
        ("01-reading-guide.md", "整理資料1: 読み方"),
        ("02-procedure.md", "整理資料2: 進め方"),
        ("03-criteria.md", "整理資料3: 判断基準"),
    ] {
        add(
            format!("library/library-organizing/{file}"),
            GROUP_LIBRARY,
            label.into(),
            organizing.join(file),
        );
    }

    add("ledger/tags.yaml".into(), GROUP_LEDGER, "タグ台帳(tags.yaml)".into(), system.join("tags.yaml"));
    add(
        "ledger/projects.yaml".into(),
        GROUP_LEDGER,
        "プロジェクト台帳(projects.yaml)".into(),
        system.join("projects.yaml"),
    );

    add(
        "profile/tier1-profile.md".into(),
        GROUP_PROFILE,
        "起動時に読むプロフィール(tier1-profile.md)".into(),
        system.join("tier1-profile.md"),
    );

    add(
        "prompts/system-prompt.md".into(),
        GROUP_BEHAVIOR,
        "システムプロンプト(system-prompt.md)".into(),
        prompts.join("system-prompt.md"),
    );
    for p in json_files(&prompts.join("tools")) {
        let name = file_name_of(&p);
        add(format!("prompts/tools/{name}"), GROUP_BEHAVIOR, format!("ツール定義({name})"), p);
    }
    for p in json_files(&prompts.join("transforms")) {
        let name = file_name_of(&p);
        add(format!("prompts/transforms/{name}"), GROUP_BEHAVIOR, format!("整形ルール({name})"), p);
    }
    add("config/config.json".into(), GROUP_BEHAVIOR, "設定(config.json)".into(), project_root.join("config.json"));

    list
}

pub fn list(library_root: &Path, project_root: &Path) -> Vec<SystemDocDto> {
    entries(library_root, project_root)
        .into_iter()
        .map(|e| {
            let meta = std::fs::metadata(&e.path).ok().filter(|m| m.is_file());
            SystemDocDto {
                format: format_of(&e.path).to_string(),
                file_name: file_name_of(&e.path),
                exists: meta.is_some(),
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                mtime: meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                id: e.id,
                group: e.group.to_string(),
                label: e.label,
            }
        })
        .collect()
}

pub fn read(library_root: &Path, project_root: &Path, id: &str) -> Result<String, String> {
    let entry = entries(library_root, project_root)
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| format!("規約画面で読める書類ではありません: {id}"))?;
    let meta = std::fs::metadata(&entry.path)
        .ok()
        .filter(|m| m.is_file())
        .ok_or_else(|| format!("ファイルが見つかりません: {}", entry.path.display()))?;
    if meta.len() > MAX_BYTES {
        return Err(format!("大きすぎて表示できません({}バイト): {}", meta.len(), entry.path.display()));
    }
    std::fs::read_to_string(&entry.path)
        .map_err(|e| format!("読み込みに失敗({}): {e}", entry.path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // library/とportable直下(config.jsonとprompts/)を再現した一時フォルダ。
    fn fixture(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("shiori-system-docs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let library = base.join("library");
        let root = base.join("portable");
        std::fs::create_dir_all(library.join("_system").join("library-organizing")).unwrap();
        std::fs::create_dir_all(root.join("prompts").join("tools")).unwrap();
        std::fs::create_dir_all(root.join("prompts").join("transforms")).unwrap();
        (base, library, root)
    }

    #[test]
    fn 許可リストはグループの順に並び不在のファイルはexistsがfalseになる() {
        let (base, library, root) = fixture("list");
        std::fs::write(library.join("_system").join("CLAUDE.md"), "# 規約").unwrap();
        std::fs::write(root.join("prompts").join("tools").join("search_knowledge.json"), "{}").unwrap();

        let docs = list(&library, &root);
        let groups: Vec<&str> = docs.iter().map(|d| d.group.as_str()).collect();
        let mut dedup = groups.clone();
        dedup.dedup();
        assert_eq!(dedup, [GROUP_MEMORY, GROUP_LIBRARY, GROUP_LEDGER, GROUP_PROFILE, GROUP_BEHAVIOR]);

        let claude = docs.iter().find(|d| d.id == "library/CLAUDE.md").unwrap();
        assert!(claude.exists && claude.size > 0 && claude.format == "markdown");
        let tags = docs.iter().find(|d| d.id == "ledger/tags.yaml").unwrap();
        assert!(!tags.exists && tags.format == "yaml");
        assert!(docs.iter().any(|d| d.id == "prompts/tools/search_knowledge.json" && d.exists));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn プロンプトのjsonは名前順で並び隠しファイルと対象外の拡張子は除く() {
        let (base, library, root) = fixture("json");
        let transforms = root.join("prompts").join("transforms");
        for name in ["query-normalization.json", "person-correction.json", "._person-correction.json", ".hidden.json", "memo.txt"] {
            std::fs::write(transforms.join(name), "{}").unwrap();
        }
        let ids: Vec<String> = list(&library, &root)
            .into_iter()
            .filter(|d| d.id.starts_with("prompts/transforms/"))
            .map(|d| d.id)
            .collect();
        assert_eq!(
            ids,
            ["prompts/transforms/person-correction.json", "prompts/transforms/query-normalization.json"]
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn 読めるのは許可リストのidだけでパスの指定は通らない() {
        let (base, library, root) = fixture("read");
        std::fs::write(library.join("_system").join("CLAUDE.md"), "# 規約").unwrap();
        std::fs::write(base.join("secret.txt"), "秘密").unwrap();

        assert_eq!(read(&library, &root, "library/CLAUDE.md").unwrap(), "# 規約");
        for bad in ["../secret.txt", "library/../../secret.txt", "secret.txt", "", "library/CLAUDE.md/.."] {
            assert!(read(&library, &root, bad).is_err(), "{bad}は読めてはいけない");
        }
        // 許可リストにあっても、ファイルが無ければエラー。
        assert!(read(&library, &root, "ledger/tags.yaml").is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn 上限を超えるファイルは読まない() {
        let (base, library, root) = fixture("big");
        std::fs::write(root.join("config.json"), vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(read(&library, &root, "config/config.json").unwrap_err().contains("大きすぎ"));
        std::fs::remove_dir_all(&base).unwrap();
    }

    // memoryを置いたlibraryの一時フォルダ。updatedを指定して作る。
    fn write_memory(library: &Path, dir: &str, project: &str, updated: Option<&str>) {
        let folder = library.join(dir).join(project);
        std::fs::create_dir_all(&folder).unwrap();
        let fm = match updated {
            Some(u) => format!("---
title: {project}
updated: {u}
---
本文"),
            None => format!("---
title: {project}
---
本文"),
        };
        std::fs::write(folder.join("memory.md"), fm).unwrap();
    }

    #[test]
    fn memoryは全体用が先頭でプロジェクトはupdatedの新しい順に並ぶ() {
        let (base, library, root) = fixture("memory-order");
        std::fs::create_dir_all(library.join("40-profile")).unwrap();
        std::fs::write(library.join("40-profile").join("memory-global.md"), "---
updated: 2026-10-01T00:00:00+09:00
---
全体").unwrap();
        write_memory(&library, "20-areas", "old", Some("2026-10-02T00:00:00+09:00"));
        write_memory(&library, "10-projects", "new", Some("\"2026-10-10T00:00:00+09:00\""));
        write_memory(&library, "20-areas", "no-updated", None);
        // 隠し・_system風のフォルダ、memory.mdが無いフォルダは出さない。
        write_memory(&library, "20-areas", ".hidden", Some("2026-10-11T00:00:00+09:00"));
        std::fs::create_dir_all(library.join("20-areas").join("empty")).unwrap();

        let ids: Vec<String> = list(&library, &root)
            .into_iter()
            .filter(|d| d.group == GROUP_MEMORY)
            .map(|d| d.id)
            .collect();
        assert_eq!(
            ids,
            ["memory/global", "memory/project/new", "memory/project/old", "memory/project/no-updated"]
        );
        assert_eq!(read(&library, &root, "memory/project/new").unwrap(), "---
title: new
updated: \"2026-10-10T00:00:00+09:00\"
---
本文");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn memoryのidにパスを渡しても読めない() {
        let (base, library, root) = fixture("memory-path");
        write_memory(&library, "20-areas", "shiori", Some("2026-10-10T00:00:00+09:00"));
        std::fs::write(base.join("secret.txt"), "秘密").unwrap();
        for bad in ["memory/project/../../secret.txt", "memory/project/shiori/../x", "memory/project/", "memory/../secret.txt", "memory/project/missing"] {
            assert!(read(&library, &root, bad).is_err(), "{bad}は読めてはいけない");
        }
        assert!(read(&library, &root, "memory/project/shiori").is_ok());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
