import { useState } from "react";
import { api } from "../../api/tauri";
import ArticleDetail from "./ArticleDetail";
import Book from "./Book";
import { getFileTitle } from "../../lib/library";
import { useShioriStore } from "../../store/useShioriStore";
import type { SearchLibraryResult } from "../../types";
import "./LibraryScreen.css";

// 1回の検索呼び出しで返す件数(詩織Ver3.0、検索機能向上3-3節と揃える)。
const PAGE_SIZE = 20;

type View = { mode: "results" } | { mode: "detail"; source: string; sourceCategory: string };

// 図書館ウィンドウ(2026-08-12、デスクトップ型ウィンドウシステムの本実装3-2)。
//
// 【Ver3.0で全面刷新】以前は起動時に蔵書全件を一括取得し、カテゴリ別の棚に
// 常時並べる表示だった。今回から「検索してから結果を見る」形式に変更した
// (UI改善4-2節)。MCPサーバーがClaude Codeに提供している「検索→該当ファイルを
// 直接読む」という体験を、人間がこのウィンドウ上でセルフサービスで行える
// ようにする狙い。カテゴリ別に全件を見る使い方は、別ウィンドウ(全件閲覧画面、
// Finder風)に切り出した。スコア閾値を設けないoffset+limitページング方式
// (検索機能向上3-3節)を踏襲し、20件ずつ「さらに読む」で追加取得する。
function LibraryScreen() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchLibraryResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [view, setView] = useState<View>({ mode: "results" });

  const runSearch = (q: string, offset: number) => {
    setSearching(true);
    setSearchError(null);
    useShioriStore
      .getState()
      .ensureBackendServicesStarted()
      .catch(() => undefined)
      .then(() => api.searchLibrary(q, PAGE_SIZE, offset))
      .then((r) => {
        setResults((prev) => (offset === 0 ? r : [...prev, ...r]));
        setHasMore(r.length === PAGE_SIZE);
      })
      .catch((err) => setSearchError(String(err)))
      .finally(() => setSearching(false));
  };

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = query.trim();
    if (!trimmed) return;
    setHasSearched(true);
    setResults([]);
    runSearch(trimmed, 0);
  };

  const handleLoadMore = () => runSearch(query.trim(), results.length);

  const openDetail = (source: string, sourceCategory: string) =>
    setView({ mode: "detail", source, sourceCategory });

  if (view.mode === "detail") {
    return (
      <div className="library-screen">
        <ArticleDetail
          source={view.source}
          sourceCategory={view.sourceCategory}
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
        />
      </form>

      <div className="library-screen__scroll">
        {!hasSearched && !searching && (
          <p className="library-screen__status">検索ワードを入力してください。</p>
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
          <div className="library-screen__shelf">
            {results.map((r) => (
              <Book
                key={r.source}
                title={getFileTitle(
                  r.source,
                  r.headings.map((h) => h.heading),
                )}
                sourceCategory={r.sourceCategory}
                size="sm"
                onClick={() => openDetail(r.source, r.sourceCategory)}
              />
            ))}
          </div>
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

export default LibraryScreen;
