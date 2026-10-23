import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { api } from "../../api/tauri";
import { handleArticleLinkClick, renderMarkdown } from "../../lib/markdown";
import { useMainViewStore } from "../../store/useMainViewStore";
import type { ShelfDoc } from "../../types";
import "../library/ArticleDetail.css";
import "./ShelfScreen.css";

// frontmatter部分(先頭の---〜---)を取り除いた本文だけをレンダリングする(ArticleDetailと同じ理由)。
function stripFrontmatter(content: string): string {
  const match = content.match(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/);
  return match ? content.slice(match[0].length) : content;
}

// 知識の棚画面(Ver5.0)。出典で確かめた、正確な参照ノートでありAI共通の正本となる知識を読む。
// 調べて置いたもの(`knowledge-research`)と、学んで理解したもの(`knowledge-learning`)の両方が入る。
// 左に記事の一覧と検索、右に本文。読み取り専用で、編集はしない(棚への保存は`shiori-save`を通す)。
// 一覧と本文はファイルを直接読む。検索だけRAG(search_shelf)を使い、RAGが止まっていても閲覧はできる。
// 学習の都度ファイルが増えるので、この画面が開かれるたびと更新ボタンで読み直す。
function ShelfScreen() {
  const [docs, setDocs] = useState<ShelfDoc[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [text, setText] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [query, setQuery] = useState("");
  // 検索で見つかったファイル名(関連度の高い順)。nullは検索していない状態。
  const [hits, setHits] = useState<string[] | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searching, setSearching] = useState(false);
  const isActive = useMainViewStore((s) => s.active === "shelf");
  const shownNameRef = useRef<string | null>(null);

  useEffect(() => {
    if (isActive) setReloadKey((k) => k + 1);
  }, [isActive]);

  useEffect(() => {
    let cancelled = false;
    api
      .listShelfDocs()
      .then((list) => {
        if (cancelled) return;
        setDocs(list);
        setListError(null);
        setSelectedName((cur) => (cur && list.some((d) => d.name === cur) ? cur : (list[0]?.name ?? null)));
      })
      .catch((err) => {
        if (!cancelled) setListError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [reloadKey]);

  useEffect(() => {
    if (!selectedName) return;
    let cancelled = false;
    if (shownNameRef.current !== selectedName) {
      setText(null);
      shownNameRef.current = selectedName;
    }
    setReadError(null);
    api
      .readShelfDoc(selectedName)
      .then((t) => {
        if (!cancelled) setText(t);
      })
      .catch((err) => {
        if (!cancelled) setReadError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [selectedName, reloadKey]);

  const runSearch = (e: FormEvent) => {
    e.preventDefault();
    const q = query.trim();
    if (!q) {
      setHits(null);
      setSearchError(null);
      return;
    }
    setSearching(true);
    setSearchError(null);
    api
      .searchShelf(q)
      .then(setHits)
      .catch((err) => setSearchError(String(err)))
      .finally(() => setSearching(false));
  };

  const clearSearch = () => {
    setQuery("");
    setHits(null);
    setSearchError(null);
  };

  // 検索中は、見つかった記事だけを関連度順に並べる。
  const shownDocs = useMemo(() => {
    if (!docs) return [];
    if (!hits) return docs;
    return hits.map((name) => docs.find((d) => d.name === name)).filter((d): d is ShelfDoc => !!d);
  }, [docs, hits]);

  const selected = docs?.find((d) => d.name === selectedName) ?? null;

  return (
    <div className="shelf-screen">
      <nav className="shelf-screen__nav" aria-label="知識の棚の記事">
        <form className="shelf-screen__search" onSubmit={runSearch}>
          <input
            type="search"
            className="shelf-screen__input"
            placeholder="棚を検索"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <button type="submit" className="shelf-screen__button" disabled={searching}>
            検索
          </button>
        </form>
        <div className="shelf-screen__tools">
          <button type="button" className="shelf-screen__button" onClick={() => setReloadKey((k) => k + 1)}>
            更新
          </button>
          {hits && (
            <button type="button" className="shelf-screen__button" onClick={clearSearch}>
              検索をやめる
            </button>
          )}
        </div>
        {listError && <p className="shelf-screen__error">{listError}</p>}
        {searchError && <p className="shelf-screen__error">検索に失敗しました: {searchError}</p>}
        {!docs && !listError && <p className="shelf-screen__dim">読み込んでいます…</p>}
        {docs && docs.length === 0 && (
          <p className="shelf-screen__dim">まだ記事がありません。学習の手順(knowledge-learning)で棚に置くと、ここに出ます。</p>
        )}
        {hits && shownDocs.length === 0 && <p className="shelf-screen__dim">該当する記事がありません。</p>}
        {shownDocs.map((d) => (
          <button
            key={d.name}
            type="button"
            className={`shelf-screen__item${d.name === selectedName ? " shelf-screen__item--active" : ""}`}
            onClick={() => setSelectedName(d.name)}
          >
            <span className="shelf-screen__item-title">{d.title}</span>
            <span className="shelf-screen__item-meta">
              {d.topic && <span>{d.topic}</span>}
            </span>
          </button>
        ))}
      </nav>

      <div className="shelf-screen__main">
        {selected && (
          <header className="shelf-screen__header">
            <h2 className="shelf-screen__title">{selected.title}</h2>
            <p className="shelf-screen__meta">
              {[
                selected.topic,
                selected.learned && `学んだ日 ${selected.learned}`,
                selected.source && `出典: ${selected.source}`,
                "読み取り専用",
              ]
                .filter(Boolean)
                .join(" ・ ")}
            </p>
          </header>
        )}
        <div className="shelf-screen__scroll">
          {readError && <p className="shelf-screen__error">{readError}</p>}
          {selected && text === null && !readError && <p className="shelf-screen__dim">読み込んでいます…</p>}
          {selected && text !== null && (
            // renderMarkdownで無害化済み(詩織Ver4.0)。
            // eslint-disable-next-line react/no-danger
            <div
              className="article-detail__body"
              onClick={handleArticleLinkClick}
              dangerouslySetInnerHTML={{ __html: renderMarkdown(stripFrontmatter(text)) }}
            />
          )}
        </div>
      </div>
    </div>
  );
}

export default ShelfScreen;
