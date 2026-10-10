import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/tauri";
import { formatMtime } from "../../lib/library";
import { handleArticleLinkClick, renderMarkdown } from "../../lib/markdown";
import type { SystemDoc } from "../../types";
import "../library/ArticleDetail.css";
import "./RulesScreen.css";

// frontmatter部分(先頭の---〜---)を取り除いた本文だけをレンダリングする(ArticleDetailと同じ理由)。
function stripFrontmatter(content: string): string {
  const match = content.match(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/);
  return match ? content.slice(match[0].length) : content;
}

// jsonは読みやすく整形する。壊れていて解析できないときは、そのまま見せる。
function formatJson(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}

function formatSize(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
}

// グループの出現順(Rust側の許可リストの順)を保ったまま、書類をグループごとにまとめる。
function groupDocs(docs: SystemDoc[]): { group: string; items: SystemDoc[] }[] {
  const groups: { group: string; items: SystemDoc[] }[] = [];
  for (const doc of docs) {
    const last = groups[groups.length - 1];
    if (last && last.group === doc.group) last.items.push(doc);
    else groups.push({ group: doc.group, items: [doc] });
  }
  return groups;
}

// 規約画面(詩織Ver4.1)。詩織のシステムを決めている書類(libraryの保存規約、タグ/プロジェクトの
// 台帳、起動時のプロフィール、プロンプト、設定)を読む。これらは索引の対象外で、図書館の画面には
// 出ない。読み取り専用で、編集はしない(書き換えるのは、保存規約に従ったAIやふらるの作業)。
function RulesScreen() {
  const [docs, setDocs] = useState<SystemDoc[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [text, setText] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .listSystemDocs()
      .then((list) => {
        if (cancelled) return;
        setDocs(list);
        // 最初に見つかる書類を開いておく(保存の規約が先頭)。
        setSelectedId((cur) => cur ?? list.find((d) => d.exists)?.id ?? null);
      })
      .catch((err) => {
        if (!cancelled) setListError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!selectedId) return;
    let cancelled = false;
    setText(null);
    setReadError(null);
    api
      .readSystemDoc(selectedId)
      .then((t) => {
        if (!cancelled) setText(t);
      })
      .catch((err) => {
        if (!cancelled) setReadError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  const groups = useMemo(() => (docs ? groupDocs(docs) : []), [docs]);
  const selected = docs?.find((d) => d.id === selectedId) ?? null;

  const body = (() => {
    if (!selected || text === null) return null;
    if (selected.format === "markdown") {
      // renderMarkdownで無害化済み(詩織Ver4.0)。
      return (
        // eslint-disable-next-line react/no-danger
        <div
          className="article-detail__body"
          onClick={handleArticleLinkClick}
          dangerouslySetInnerHTML={{ __html: renderMarkdown(stripFrontmatter(text)) }}
        />
      );
    }
    return <pre className="rules-screen__code">{selected.format === "json" ? formatJson(text) : text}</pre>;
  })();

  return (
    <div className="rules-screen">
      <nav className="rules-screen__nav" aria-label="規約の書類">
        {listError && <p className="rules-screen__error">{listError}</p>}
        {!docs && !listError && <p className="rules-screen__dim">読み込んでいます…</p>}
        {groups.map(({ group, items }) => (
          <section key={group} className="rules-screen__group">
            <h3 className="rules-screen__group-title">{group}</h3>
            {items.map((d) => (
              <button
                key={d.id}
                type="button"
                disabled={!d.exists}
                className={`rules-screen__item${d.id === selectedId ? " rules-screen__item--active" : ""}`}
                onClick={() => setSelectedId(d.id)}
                title={d.exists ? d.fileName : `${d.fileName}(見つかりません)`}
              >
                {d.label}
                {!d.exists && <span className="rules-screen__missing">なし</span>}
              </button>
            ))}
          </section>
        ))}
      </nav>

      <div className="rules-screen__main">
        {selected && (
          <header className="rules-screen__header">
            <h2 className="rules-screen__title">{selected.label}</h2>
            <p className="rules-screen__meta">
              {selected.fileName} ・ {formatSize(selected.size)} ・ 更新 {formatMtime(selected.mtime)} ・ 読み取り専用
            </p>
          </header>
        )}
        <div className="rules-screen__scroll">
          {readError && <p className="rules-screen__error">{readError}</p>}
          {selected && text === null && !readError && <p className="rules-screen__dim">読み込んでいます…</p>}
          {!selected && docs && <p className="rules-screen__dim">表示できる書類がありません。</p>}
          {body}
        </div>
      </div>
    </div>
  );
}

export default RulesScreen;
