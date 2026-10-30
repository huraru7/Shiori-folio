import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/tauri";
import { formatMtime, formatRelative, isRecent } from "../../lib/library";
import { handleArticleLinkClick, renderMarkdown } from "../../lib/markdown";
import { useMainViewStore } from "../../store/useMainViewStore";
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

// 画面の見出し(タブ)。順番は画面の左から。idはRust側(system_docs.rs)の`tab`と同じ。
const TABS = [
  { id: "memory", label: "記憶" },
  { id: "skill", label: "スキル" },
  { id: "rules", label: "規約" },
  { id: "behavior", label: "詩織の動作" },
];

// 小見出し(sub)の出現順(Rust側の並び順)を保ったまま、書類を小見出しごとにまとめる。
// 記憶は「全体」を先頭に更新の新しい順で1本に並べる(領域とプロジェクトが交互に来るので、
// 小見出しで分けず、各行の種類の表示で見分ける)。
function groupDocs(docs: SystemDoc[], tab: string): { sub: string; items: SystemDoc[] }[] {
  if (tab === "memory") return docs.length ? [{ sub: "", items: docs }] : [];
  const groups: { sub: string; items: SystemDoc[] }[] = [];
  for (const doc of docs) {
    const last = groups[groups.length - 1];
    if (last && last.sub === doc.sub) last.items.push(doc);
    else groups.push({ sub: doc.sub, items: [doc] });
  }
  return groups;
}

// 書類の更新時刻(ミリ秒)。memoryはfrontmatterのupdated、それ以外はファイルの更新日時。
function docTimeMs(doc: SystemDoc): number | null {
  if (doc.updated) {
    const t = Date.parse(doc.updated);
    if (!Number.isNaN(t)) return t;
  }
  return doc.mtime ? doc.mtime * 1000 : null;
}

// 規約・記憶画面(詩織Ver4.1、memoryの閲覧はVer4.2、見出しで分けたのはVer6.0.2)。上の見出し
// (記憶・スキル・規約・詩織の動作)で書類を種類ごとに分け、左の一覧には選んだ見出しの書類だけを出す。
// 詩織のシステムを決めている書類(libraryの
// 保存規約、タグ/プロジェクトの台帳、起動時のプロフィール、プロンプト、設定)と、共通記憶MD(全体用・
// 各プロジェクトのmemory)を読む。これらは図書館の画面には出ない。読み取り専用で、編集はしない
// (書き換えるのは、保存規約に従ったAIやふらるの作業。memoryは`shiori-save`経由のみ)。
// memoryは作業中に書き換わるので、この画面が開かれるたび、と更新ボタンで読み直す(画面は常に
// マウントされたままなので、初回だけの読み込みでは古いままになる)。
function RulesScreen() {
  const [docs, setDocs] = useState<SystemDoc[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [tab, setTab] = useState("memory");
  // 見出しごとに、最後に選んだ書類を覚える(見出しを行き来しても選択が戻る)。
  const [selectedByTab, setSelectedByTab] = useState<Record<string, string>>({});
  // 開いているスキル(子の参照ファイルを持つ親のid)。
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [text, setText] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  // 読み直しの合図。更新ボタンと、この画面が開かれたときに増やす。
  const [reloadKey, setReloadKey] = useState(0);
  const isActive = useMainViewStore((s) => s.active === "rules");
  // 本文を読み直すとき、同じ書類のままなら、読み込み中の表示に戻さず今の本文を見せ続ける。
  const shownIdRef = useRef<string | null>(null);

  useEffect(() => {
    if (isActive) setReloadKey((k) => k + 1);
  }, [isActive]);

  useEffect(() => {
    let cancelled = false;
    api
      .listSystemDocs()
      .then((list) => {
        if (cancelled) return;
        setDocs(list);
      })
      .catch((err) => {
        if (!cancelled) setListError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [reloadKey]);

  const tabDocs = useMemo(() => (docs ?? []).filter((d) => d.tab === tab), [docs, tab]);
  // 見出しで選んだ書類。まだ選んでいなければ、その見出しの最初に読める書類(記憶なら「全体」)。
  const selectedId =
    (selectedByTab[tab] && tabDocs.some((d) => d.id === selectedByTab[tab]) ? selectedByTab[tab] : null) ??
    tabDocs.find((d) => d.exists)?.id ??
    null;

  useEffect(() => {
    if (!selectedId) return;
    let cancelled = false;
    if (shownIdRef.current !== selectedId) {
      setText(null);
      shownIdRef.current = selectedId;
    }
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
  }, [selectedId, reloadKey]);

  const selected = docs?.find((d) => d.id === selectedId) ?? null;
  // 子の参照ファイルを持つ親のid → 子の一覧。
  const childrenOf = useMemo(() => {
    const map = new Map<string, SystemDoc[]>();
    for (const d of tabDocs) {
      if (d.parentId) map.set(d.parentId, [...(map.get(d.parentId) ?? []), d]);
    }
    return map;
  }, [tabDocs]);
  const parentDoc = selected?.parentId ? (docs?.find((d) => d.id === selected.parentId) ?? null) : null;
  // 閉じている親の子は一覧に出さない(選んでいる子は、見失わないよう出しておく)。
  const groups = useMemo(
    () =>
      groupDocs(
        tabDocs.filter((d) => !d.parentId || expanded.has(d.parentId) || d.id === selectedId),
        tab,
      ),
    [tabDocs, expanded, selectedId, tab],
  );

  // 書類を選ぶ。親を選ぶと子を開き、子を選ぶと親を開いておく(一覧で位置が分かるように)。
  const selectDoc = (d: SystemDoc) => {
    setSelectedByTab((cur) => ({ ...cur, [d.tab]: d.id }));
    const open = d.parentId ?? (childrenOf.has(d.id) ? d.id : null);
    if (open) setExpanded((cur) => new Set(cur).add(open));
  };
  const toggleExpanded = (id: string) =>
    setExpanded((cur) => {
      const next = new Set(cur);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const selectedTimeMs = selected ? docTimeMs(selected) : null;

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
      <div className="rules-screen__tabs" role="tablist" aria-label="規約・記憶の見出し">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={t.id === tab}
            className={`rules-screen__tab rules-screen__tab--${t.id}${t.id === tab ? " rules-screen__tab--active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
            <span className="rules-screen__tab-count">
              {(docs ?? []).filter((d) => d.tab === t.id && d.exists).length}
            </span>
          </button>
        ))}
        <button type="button" className="rules-screen__refresh" onClick={() => setReloadKey((k) => k + 1)}>
          更新
        </button>
      </div>

      <div className={`rules-screen__body rules-screen__body--${tab}`}>
        <nav className="rules-screen__nav" aria-label="規約・記憶の書類">
          {listError && <p className="rules-screen__error">{listError}</p>}
          {!docs && !listError && <p className="rules-screen__dim">読み込んでいます…</p>}
          {groups.map(({ sub, items }) => (
            <section key={sub} className="rules-screen__group">
              {sub && <h3 className="rules-screen__group-title">{sub}</h3>}
              {items.map((d) => {
                const timeMs = docTimeMs(d);
                const hasChildren = childrenOf.has(d.id);
                return (
                  <div key={d.id} className={`rules-screen__row${d.parentId ? " rules-screen__row--child" : ""}`}>
                    {tab === "skill" &&
                      (hasChildren ? (
                        <button
                          type="button"
                          className="rules-screen__toggle"
                          aria-expanded={expanded.has(d.id)}
                          aria-label={`${d.label}の参照ファイルを${expanded.has(d.id) ? "閉じる" : "開く"}`}
                          onClick={() => toggleExpanded(d.id)}
                        >
                          {expanded.has(d.id) ? "▾" : "▸"}
                        </button>
                      ) : (
                        <span className="rules-screen__toggle-spacer" />
                      ))}
                    <button
                      type="button"
                      disabled={!d.exists}
                      className={`rules-screen__item${d.id === selectedId ? " rules-screen__item--active" : ""}`}
                      onClick={() => selectDoc(d)}
                      title={d.exists ? `${d.label}(${d.fileName})` : `${d.fileName}(見つかりません)`}
                    >
                      <span className="rules-screen__item-label">{d.label}</span>
                      {tab === "memory" && <span className="rules-screen__item-kind">{d.sub}</span>}
                      {!d.exists && <span className="rules-screen__missing">なし</span>}
                      {d.exists && timeMs !== null && (
                        <span
                          className={`rules-screen__item-time${isRecent(timeMs) ? " rules-screen__item-time--recent" : ""}`}
                        >
                          {formatRelative(timeMs)}
                        </span>
                      )}
                    </button>
                  </div>
                );
              })}
            </section>
          ))}
          {docs && groups.length === 0 && <p className="rules-screen__dim">この見出しの書類はありません。</p>}
        </nav>

        <div className="rules-screen__main">
          {selected && (
            <header className="rules-screen__header">
              <h2 className="rules-screen__title">
                {parentDoc && <span className="rules-screen__title-parent">{parentDoc.label} › </span>}
                {selected.label}
              </h2>
              <p className="rules-screen__meta">
                {selected.fileName} ・ {formatSize(selected.size)}
                {selectedTimeMs !== null &&
                  ` ・ ${selected.updated ? "最終更新(updated)" : "更新"} ${formatRelative(selectedTimeMs)}(${formatMtime(selectedTimeMs / 1000)})`}
                {" ・ 読み取り専用"}
              </p>
            </header>
          )}
          <div className="rules-screen__scroll">
            {readError && <p className="rules-screen__error">{readError}</p>}
            {selected && text === null && !readError && <p className="rules-screen__dim">読み込んでいます…</p>}
            {!selected && docs && <p className="rules-screen__dim">表示できる書類がありません。</p>}
            {body}
            {text !== null && selected && (parentDoc || childrenOf.has(selected.id)) && (
              <footer className="rules-screen__links">
                <h3 className="rules-screen__links-title">{parentDoc ? "親のスキル" : "参照ファイル"}</h3>
                {(parentDoc ? [parentDoc] : (childrenOf.get(selected.id) ?? [])).map((d) => (
                  <button
                    key={d.id}
                    type="button"
                    disabled={!d.exists}
                    className="rules-screen__link"
                    onClick={() => selectDoc(d)}
                  >
                    {parentDoc ? `‹ ${d.label}` : `${d.label} ›`}
                  </button>
                ))}
              </footer>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default RulesScreen;
