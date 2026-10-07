import { useEffect, useRef, useState } from "react";
import { api } from "../../api/tauri";
import ArticleDetail from "./ArticleDetail";
import { CATEGORY_ORDER, getCategoryMeta, getFileTitle, isAssetMeta } from "../../lib/library";
import { useMainViewStore } from "../../store/useMainViewStore";
import { useShioriStore } from "../../store/useShioriStore";
import type { LibraryFilter, SearchLibraryResult } from "../../types";
import shioriMark from "../../assets/logo/shiori-mark-master.png";
import "./LibraryScreen.css";

// 1回の検索呼び出しで返す件数(詩織Ver3.0、検索機能向上3-3節と揃える)。
const PAGE_SIZE = 20;

// 古くなった記録として、一覧で分かるようにするstatus。
const STALE_STATUSES = ["outdated", "deprecated"];
const ARCHIVE_CATEGORY = "90-archive";

// 記事の種類(frontmatterのkind)。libraryの規約(_system/CLAUDE.md)ではこの2つだけ。
const KIND_OPTIONS = [
  { value: "journal", label: "作業記録" },
  { value: "resource", label: "資料" },
];

const DEFAULT_FILTER: LibraryFilter = { includeStale: false, sourceCategories: [], project: null, kind: null };

type View =
  | { mode: "results" }
  | { mode: "detail"; source: string; sourceCategory: string; assetPath: string };

// 図書館画面(2026-08-12、詩織Ver3.9で右ゾーンのタブの1つにした)。
//
// 【Ver3.0で全面刷新】以前は起動時に蔵書全件を一括取得し、カテゴリ別の棚に
// 常時並べる表示だった。今回から「検索してから結果を見る」形式に変更した
// (UI改善4-2節)。MCPサーバーがClaude Codeに提供している「検索→該当ファイルを
// 直接読む」という体験を、人間がこの画面上でセルフサービスで行えるようにする
// 狙い。カテゴリ別に全件を見る使い方は、全件閲覧画面(Finder風)に切り出した。
// スコア閾値を設けないoffset+limitページング方式(検索機能向上3-3節)を踏襲し、
// 20件ずつ「さらに読む」で追加取得する。
//
// 【Ver3.9】結果は本の背表紙ではなく、検索エンジンの結果のように「タイトル・
// 属性・冒頭の文」の一覧で出す。検索欄の×とEscで最初の状態に戻せる。
// 人はAIほど多くを読めないため、アーカイブと古い記録は既定で除き、棚・project・
// 種類でも絞り込める(絞り込みはRAG側で行うので、ページングも正しく動く)。
function LibraryScreen() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchLibraryResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [view, setView] = useState<View>({ mode: "results" });
  const [filter, setFilter] = useState<LibraryFilter>(DEFAULT_FILTER);
  const [projects, setProjects] = useState<string[]>([]);
  const libraryQuery = useMainViewStore((s) => s.libraryQuery);
  // リセット後に、それより前に始めた検索の結果が遅れて届いても反映しないため。
  const generationRef = useRef(0);

  const runSearch = (q: string, offset: number, f: LibraryFilter = filter) => {
    const generation = generationRef.current;
    const isCurrent = () => generation === generationRef.current;
    setSearching(true);
    setSearchError(null);
    useShioriStore
      .getState()
      .ensureBackendServicesStarted()
      .catch(() => undefined)
      .then(() => api.searchLibrary(q, PAGE_SIZE, offset, f))
      .then((r) => {
        if (!isCurrent()) return;
        setResults((prev) => (offset === 0 ? r : [...prev, ...r]));
        setHasMore(r.length === PAGE_SIZE);
      })
      .catch((err) => {
        if (isCurrent()) setSearchError(String(err));
      })
      .finally(() => {
        if (isCurrent()) setSearching(false);
      });
  };

  const startSearch = (q: string, f: LibraryFilter = filter) => {
    const trimmed = q.trim();
    if (!trimmed) return;
    generationRef.current += 1;
    setHasSearched(true);
    setResults([]);
    setView({ mode: "results" });
    runSearch(trimmed, 0, f);
  };

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    startSearch(query);
  };

  // 絞り込みを変えたら、検索済みなら同じ検索語で検索し直す。
  const changeFilter = (next: LibraryFilter) => {
    setFilter(next);
    if (hasSearched) startSearch(query, next);
  };

  const toggleCategory = (category: string) => {
    const selected = filter.sourceCategories.includes(category)
      ? filter.sourceCategories.filter((c) => c !== category)
      : [...filter.sourceCategories, category];
    changeFilter({ ...filter, sourceCategories: selected });
  };

  useEffect(() => {
    api
      .listLibraryProjects()
      .then(setProjects)
      .catch(() => {
        // 選択肢が出せないだけで、検索自体はできるので無視する。
      });
  }, []);

  // ホームの検索欄から渡された検索語で検索する。
  useEffect(() => {
    if (libraryQuery === null) return;
    useMainViewStore.getState().clearLibraryQuery();
    setQuery(libraryQuery);
    startSearch(libraryQuery);
    // startSearchは毎回作り直されるが、検索語が届いたときだけ動かしたい。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [libraryQuery]);

  const reset = () => {
    generationRef.current += 1;
    setQuery("");
    setResults([]);
    setSearching(false);
    setSearchError(null);
    setHasSearched(false);
    setHasMore(false);
    setView({ mode: "results" });
  };

  const handleLoadMore = () => runSearch(query.trim(), results.length);

  const openDetail = (source: string, sourceCategory: string, assetPath: string) =>
    setView({ mode: "detail", source, sourceCategory, assetPath });

  if (view.mode === "detail") {
    return (
      <div className="library-screen">
        <ArticleDetail
          source={view.source}
          sourceCategory={view.sourceCategory}
          assetPath={view.assetPath}
          onBack={() => setView({ mode: "results" })}
        />
      </div>
    );
  }

  return (
    <div className="library-screen">
      <form className="library-screen__search" onSubmit={handleSearchSubmit}>
        🔍
        <input
          type="text"
          placeholder="蔵書を検索…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") reset();
          }}
        />
        {(query || hasSearched) && (
          <button
            type="button"
            className="library-screen__clear"
            onClick={reset}
            title="検索をリセット(Esc)"
            aria-label="検索をリセット"
          >
            ×
          </button>
        )}
      </form>

      <div className="library-filter">
        <label className="library-filter__toggle">
          <input
            type="checkbox"
            checked={filter.includeStale}
            onChange={(e) =>
              changeFilter({
                ...filter,
                includeStale: e.target.checked,
                sourceCategories: e.target.checked
                  ? filter.sourceCategories
                  : filter.sourceCategories.filter((c) => c !== ARCHIVE_CATEGORY),
              })
            }
          />
          アーカイブと古い記録を含める
        </label>
        <div className="library-filter__chips" role="group" aria-label="棚">
          {CATEGORY_ORDER.filter((c) => filter.includeStale || c !== ARCHIVE_CATEGORY).map((c) => {
            const meta = getCategoryMeta(c);
            const selected = filter.sourceCategories.includes(c);
            return (
              <button
                key={c}
                type="button"
                aria-pressed={selected}
                className={`library-filter__chip${selected ? " library-filter__chip--active" : ""}`}
                style={selected ? { color: meta.hex, borderColor: meta.hex } : undefined}
                onClick={() => toggleCategory(c)}
              >
                {meta.label}
              </button>
            );
          })}
        </div>
        <select
          className="library-filter__select"
          aria-label="project"
          value={filter.project ?? ""}
          onChange={(e) => changeFilter({ ...filter, project: e.target.value || null })}
        >
          <option value="">すべてのproject</option>
          {projects.map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </select>
        <select
          className="library-filter__select"
          aria-label="種類"
          value={filter.kind ?? ""}
          onChange={(e) => changeFilter({ ...filter, kind: e.target.value || null })}
        >
          <option value="">すべての種類</option>
          {KIND_OPTIONS.map((k) => (
            <option key={k.value} value={k.value}>
              {k.label}
            </option>
          ))}
        </select>
      </div>

      <div className="library-screen__scroll">
        {!hasSearched && !searching && (
          <div className="library-screen__empty">
            <img src={shioriMark} className="library-screen__watermark" alt="" />
            <p className="library-screen__status">検索ワードを入力してください。</p>
          </div>
        )}
        {searching && results.length === 0 && (
          <p className="library-screen__status">検索しています…</p>
        )}
        {searchError && (
          <div className="library-screen__status library-screen__status--error">
            <p>{searchError}</p>
            <button
              type="button"
              className="library-screen__retry-btn"
              onClick={() => runSearch(query.trim(), 0)}
            >
              再試行
            </button>
          </div>
        )}
        {hasSearched && !searching && !searchError && results.length === 0 && (
          <p className="library-screen__status">該当する蔵書が見つかりませんでした。</p>
        )}

        {results.length > 0 && (
          <ol className="library-results">
            {results.map((r) => (
              <SearchResultItem
                key={r.source}
                result={r}
                onOpen={() => openDetail(r.source, r.sourceCategory, r.assetPath)}
              />
            ))}
          </ol>
        )}

        {hasMore && !searching && (
          <button type="button" className="library-screen__load-more" onClick={handleLoadMore}>
            さらに読む
          </button>
        )}
        {searching && results.length > 0 && (
          <p className="library-screen__status">読み込んでいます…</p>
        )}
      </div>
    </div>
  );
}

function SearchResultItem({ result, onOpen }: { result: SearchLibraryResult; onOpen: () => void }) {
  const category = getCategoryMeta(result.sourceCategory);
  const title =
    result.title ||
    getFileTitle(
      result.source,
      result.headings.map((h) => h.heading),
    );
  const attributes = [result.project, result.entryKind, result.date].filter(Boolean);
  return (
    <li className="library-result">
      <div className="library-result__meta">
        <span className="library-result__chip" style={{ color: category.hex, borderColor: category.hex }}>
          {category.label}
        </span>
        {isAssetMeta(result.source) && <span className="library-result__chip">素材</span>}
        {STALE_STATUSES.includes(result.status) && (
          <span className="library-result__chip library-result__chip--stale">古い記録</span>
        )}
        {attributes.length > 0 && <span className="library-result__attrs">{attributes.join(" · ")}</span>}
      </div>
      <button type="button" className="library-result__title" onClick={onOpen} title={title}>
        {title}
      </button>
      {result.summary && <p className="library-result__summary">{result.summary}</p>}
    </li>
  );
}

export default LibraryScreen;
