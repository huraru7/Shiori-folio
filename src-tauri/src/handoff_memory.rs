//! ホームの「申し送り」を、共通記憶MD(kind: memory、詩織Ver4.1)から作るための解析。
//!
//! 以前は、最も新しいjournalの「申し送り」の節を拾っていた。journalは「その日の記録」で、
//! 片付いた後も残り、プロジェクトもまたいで選ばれる。memoryの「やること」「申し送り・待ち」は、
//! 完了したら削除される「いま有効な分」の正本なので、ホームはそちらを出す。
//! ここは文字列の解析だけで、ファイルの選択(updatedが最も新しいmemory)はlib.rs側で行う。

/// memoryの「やること」と「申し送り・待ち」の項目(飾りを外した1行ずつ)。
#[derive(Debug, Default, PartialEq)]
pub struct MemorySections {
    pub todos: Vec<String>,
    pub notes: Vec<String>,
}

/// library/からの相対パスが、プロジェクト単位のmemory(`10-projects/{project}/memory.md`・
/// `20-areas/{project}/memory.md`)か。全体用の`40-profile/memory-global.md`は含めない。
pub fn is_project_memory_path(relative_path: &str) -> bool {
    let normalized = relative_path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').collect();
    parts.len() == 3 && matches!(parts[0], "10-projects" | "20-areas") && parts[2] == "memory.md"
}

enum Section {
    Todos,
    Notes,
}

// 太字・コード・リンクの飾りと、チェックボックス(`[ ] `・`[x] `)を外した1行にする。
fn clean_item(raw: &str) -> String {
    let text = raw.replace("**", "").replace('`', "").replace("[[", "").replace("]]", "");
    let text = text.trim();
    for prefix in ["[ ] ", "[x] ", "[X] "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    text.to_string()
}

/// 「やること」の見出しの節と、「申し送り」を含む見出しの節から、先頭階層の箇条書きを取り出す。
/// 入れ子の項目(字下げした行)は、親の補足なので拾わない。
pub fn memory_sections(content: &str) -> MemorySections {
    let mut sections = MemorySections::default();
    let mut current: Option<Section> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let title = trimmed.trim_start_matches('#').trim();
            current = if title == "やること" {
                Some(Section::Todos)
            } else if title.contains("申し送り") {
                Some(Section::Notes)
            } else {
                None
            };
            continue;
        }
        let Some(section) = &current else { continue };
        let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) else {
            continue;
        };
        let item = clean_item(item);
        if item.is_empty() {
            continue;
        }
        match section {
            Section::Todos => sections.todos.push(item),
            Section::Notes => sections.notes.push(item),
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMORY: &str = "---\ntitle: 詩織の共通記憶\nkind: memory\n---\n\n## 概要\n\n- これは概要なので拾わない\n\n## 現在の状態\n\n- 状態も拾わない\n\n## やること\n\n- [ ] Macで **ビルド** する。(10/10)\n- [x] 済んだもの\n  - 入れ子は拾わない\n- 通常の箇条書き\n\n## 申し送り・待ち\n\n- `tags.yaml` に [[設計]] が無い。(10/10)\n- \n\n## その他\n\n- これも拾わない\n";

    #[test]
    fn やることと申し送りの先頭階層の項目だけを飾りとチェックボックスを外して取り出す() {
        let s = memory_sections(MEMORY);
        assert_eq!(s.todos, ["Macで ビルド する。(10/10)", "済んだもの", "通常の箇条書き"]);
        assert_eq!(s.notes, ["tags.yaml に 設計 が無い。(10/10)"]);
    }

    #[test]
    fn 該当する見出しが無ければ空になる() {
        assert_eq!(memory_sections("## 概要\n\n- だけ\n"), MemorySections::default());
        assert_eq!(memory_sections(""), MemorySections::default());
    }

    #[test]
    fn プロジェクト単位のmemoryのパスだけを判定する() {
        for ok in ["20-areas/shiori/memory.md", "10-projects/tanker/memory.md", "20-areas\\shiori\\memory.md"] {
            assert!(is_project_memory_path(ok), "{ok}は対象");
        }
        for ng in [
            "40-profile/memory-global.md",
            "20-areas/shiori/journal/memory.md",
            "90-archive/20-areas/shiori/memory.md",
            "20-areas/shiori/memory.md.bak",
            "30-resources/memory.md",
            "memory.md",
        ] {
            assert!(!is_project_memory_path(ng), "{ng}は対象外");
        }
    }
}
