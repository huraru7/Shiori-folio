import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/tauri";
import ArticleDetail from "./ArticleDetail";
import Book from "./Book";
import { CATEGORY_ORDER, getCategoryMeta, getFileCallNo, getFileTitle } from "../../lib/library";
import { useShioriStore } from "../../store/useShioriStore";
import type { LibraryFile } from "../../types";
import "./LibraryBrowseScreen.css";

// 起動直後の一時的な接続断を自動で吸収するためのリトライ設定
// (LibraryScreenのPhase4修正と同じ方針、詩織Ver3.0 UI改善4-2節)。
const MAX_AUTO_RETRIES = 5;
const RETRY_INTERVAL_MS = 3000;

type View =
  | { mode: "list" }
  | { mode: "detail"; source: string; sourceCategory: string; callNo: string };

// 全件閲覧ウィンドウ(詩織Ver3.0、UI改善4-2節「全件閲覧画面の新設」)。
// ライブラリウィンドウ(LibraryScreen)が検索結果ベースのUIに変わったことに
// 伴い、旧来の「カテゴリ別に全件を見る」使い方をこちらへ切り出した
// (Finder/Explorer風のエクスプローラー形式)。起動時の自動ロード・接続断
// リトライのロジックは旧LibraryScreen(Phase4修正)からそのまま移設している。
function LibraryBrowseScreen() {
  const [files, setFiles] = useState<LibraryFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const [manualRetryKey, setManualRetryKey] = useState(0);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<View>({ mode: "list" });

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const attemptFetch = (attempt: number) => {
      setLoading(true);
      setError(null);
      useShioriStore
        .getState()
        .ensureBackendServicesStarted()
        .catch(() => undefined)
        .then(() => api.listAllKnowledge())
        .then((r) => {
          if (cancelled) return;
          setFiles(r);
          setLoading(false);
        })
        .catch((err) => {
          if (cancelled) return;
          if (attempt < MAX_AUTO_RETRIES) {
            setRetryAttempt(attempt);
            timer = setTimeout(() => {
              if (!cancelled) attemptFetch(attempt + 1);
            }, RETRY_INTERVAL_MS);
          } else {
            setError(String(err));
            setLoading(false);
          }
        });
    };

    setRetryAttempt(0);
    attemptFetch(1);
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [manualRetryKey]);

  const handleManualRetry = () => setManualRetryKey((k) => k + 1);

  // 意味検索(LibraryScreenのsearch_library)とは別の、ファイル名・見出しの
  // 単純な部分一致による簡易検索(UI改善4-2節)。
  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return files;
    return files.filter(
      (f) =>
        f.source.toLowerCase().includes(normalized) ||
        f.headings.some((h) => h.toLowerCase().includes(normalized)),
    );
  }, [files, query]);

  // フォルダ階層(カテゴリ)がそのまま見える構成にするため、カテゴリ別に
  // グルーピングする。
  const grouped = useMemo(() => {
    const byCategory = new Map<string, LibraryFile[]>();
    for (const file of filtered) {
      const list = byCategory.get(file.sourceCategory) ?? [];
      list.push(file);
      byCategory.set(file.sourceCategory, list);
    }
    const orderedKeys = [
      ...CATEGORY_ORDER.filter((c) => byCategory.has(c)),
      ...[...byCategory.keys()].filter((c) => !(CATEGORY_ORDER as readonly string[]).includes(c)),
    ];
    return orderedKeys.map((category) => ({ category, items: byCategory.get(category)! }));
  }, [filtered]);

  if (view.mode === "detail") {
    return (
      <div className="library-browse-screen">
        <ArticleDetail
          source={view.source}
          sourceCategory={view.sourceCategory}
          callNo={view.callNo}
          onBack={() => setView({ mode: "list" })}
        />
      </div>
    );
  }

  return (
    <div className="library-browse-screen">
      <div className="library-browse-screen__search">
        🔍
        <input
          type="text"
          placeholder="ファイル名・見出しで絞り込む…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      <div className="library-browse-screen__scroll">
        {loading && (
          <p className="library-browse-screen__status">
            蔵書を読み込んでいます…
            {retryAttempt > 0 && `(再試行 ${retryAttempt}/${MAX_AUTO_RETRIES})`}
          </p>
        )}
        {error && !loading && (
          <div className="library-browse-screen__status library-browse-screen__status--error">
            <p>{error}</p>
            <button
              type="button"
              className="library-browse-screen__retry-btn"
              onClick={handleManualRetry}
            >
              再試行
            </button>
          </div>
        )}
        {!loading && !error && grouped.length === 0 && (
          <p className="library-browse-screen__status">該当する蔵書が見つかりませんでした。</p>
        )}

        {grouped.map(({ category, items }) => {
          const meta = getCategoryMeta(category);
          return (
            <div className="library-browse-screen__group" key={category}>
              <div className="library-browse-screen__group-head">
                <span className="library-browse-screen__swatch" style={{ background: meta.hex }} />
                {meta.label}({items.length})
              </div>
              <div className="library-browse-screen__shelf">
                {items.map((file, i) => (
                  <Book
                    key={file.source}
                    title={getFileTitle(file.source, file.headings)}
                    sourceCategory={file.sourceCategory}
                    size="sm"
                    onClick={() =>
                      setView({
                        mode: "detail",
                        source: file.source,
                        sourceCategory: file.sourceCategory,
                        callNo: getFileCallNo(i, file.sourceCategory),
                      })
                    }
                  />
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default LibraryBrowseScreen;
