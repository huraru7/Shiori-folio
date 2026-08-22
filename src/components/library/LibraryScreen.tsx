import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/tauri";
import SourceDocumentModal from "../panels/SourceDocumentModal";
import Book from "./Book";
import FileModal from "./FileModal";
import { CATEGORY_ORDER, getCategoryMeta, getFileCallNo, getFileTitle } from "../../lib/library";
import { useShioriStore } from "../../store/useShioriStore";
import type { LibraryFile } from "../../types";
import "./LibraryScreen.css";

// 起動直後の一時的な接続断(RAGサーバーの起動待ち)を自動で吸収するための
// リトライ設定(詩織Ver3.0、UI改善4-2節)。3秒間隔で最大5回試行し、それでも
// 失敗する場合は手動リトライボタン付きのエラー表示に切り替える。
const MAX_AUTO_RETRIES = 5;
const RETRY_INTERVAL_MS = 3000;

// 図書館ウィンドウ(2026-08-12、デスクトップ型ウィンドウシステムの本実装3-2)。
// 以前はスタンドアロンの全画面ビューだったが、OsWindow内のコンパクト版に
// 作り替えた(開閉・ドラッグ・リサイズ等はOsWindow側が担うため、ここでは
// 検索バー固定+一覧スクロールの中身だけを持つ)。検索を経由しない蔵書全件を
// 取得し、カテゴリ別の棚に並べる(Phase 7以降、1冊=1ファイルの表示単位)。
// ここでは既に棚に並んでいるものを自分の意思で見にいくだけなので、
// KnowledgePanelのgather演出は発動しない。
function LibraryScreen() {
  const [files, setFiles] = useState<LibraryFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const [manualRetryKey, setManualRetryKey] = useState(0);
  const [query, setQuery] = useState("");

  const [selected, setSelected] = useState<LibraryFile | null>(null);
  const [selectedCallNo, setSelectedCallNo] = useState("");
  const [sourceOpen, setSourceOpen] = useState(false);
  const [content, setContent] = useState<string | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);

  // DesktopArea(ひいてはLibraryScreen)はStartupScreenの完了を待たず、アプリ
  // 起動と同時に常時マウントされている(StartupScreenは上乗せのオーバーレイに
  // 過ぎない)。そのため、ここでバックエンド起動を待たずに即listAllKnowledge()を
  // 呼ぶと、RAGサーバー(Pythonプロセス)がまだ起動すらしていないタイミングで
  // 失敗することがある(2026-08-14、外付けSSD運用の実機確認で発覚)。
  //
  // 【Ver3.0で修正】以前はここで失敗すると手段が無いままエラー表示が固定化
  // され、その後RAGサーバーが正常化してもウィンドウの再オープンや再起動まで
  // 「蔵書が読み込めない」状態に固まったままになる不具合があった(UI改善
  // 4-2節)。起動直後の一時的な接続断を自動で吸収できるよう、3秒間隔で
  // 最大5回まで自動リトライし、それでも失敗した場合のみ手動リトライボタン
  // 付きのエラー表示に切り替える。manualRetryKeyを変えることでeffect自体を
  // 再実行し、手動リトライ時も同じ自動リトライ付きのフローに乗せる。
  //
  // ensureBackendServicesStarted()はモジュールスコープの共有Promiseのため、
  // ここで待ち受けても起動処理自体が重複することはない(App.tsx/StartupScreenの
  // 呼び出しと同じPromiseに相乗りするだけ)。他サービス(LLM/embedding)の起動
  // 失敗はここでは無視する(RAG自体は起動できている可能性があるため)。
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

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return files;
    return files.filter(
      (f) =>
        f.source.toLowerCase().includes(normalized) ||
        f.headings.some((h) => h.toLowerCase().includes(normalized)),
    );
  }, [files, query]);

  // カテゴリ別の棚に、1ファイル=1冊として並べる(Phase 7)。
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
    return orderedKeys.map((category) => {
      const items = byCategory.get(category)!;
      return { category, items };
    });
  }, [filtered]);

  const handleOpenFile = (file: LibraryFile, callNo: string) => {
    setSelected(file);
    setSelectedCallNo(callNo);
    setSourceOpen(false);
  };

  const handleOpenSource = () => {
    if (!selected) return;
    setSourceOpen(true);
    setContent(null);
    setSourceError(null);
    api
      .getSourceDocument(selected.sourceCategory, selected.source)
      .then(setContent)
      .catch((err) => setSourceError(String(err)));
  };

  return (
    <div className="library-screen">
      <div className="library-screen__search">
        🔍
        <input
          type="text"
          placeholder="蔵書を検索…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      <div className="library-screen__scroll">
        {loading && (
          <p className="library-screen__status">
            蔵書を読み込んでいます…
            {retryAttempt > 0 && `(再試行 ${retryAttempt}/${MAX_AUTO_RETRIES})`}
          </p>
        )}
        {error && !loading && (
          <div className="library-screen__status library-screen__status--error">
            <p>{error}</p>
            <button type="button" className="library-screen__retry-btn" onClick={handleManualRetry}>
              再試行
            </button>
          </div>
        )}
        {!loading && !error && grouped.length === 0 && (
          <p className="library-screen__status">該当する蔵書が見つかりませんでした。</p>
        )}

        {grouped.map(({ category, items }) => {
          const meta = getCategoryMeta(category);
          return (
            <div className="library-screen__group" key={category}>
              <div className="library-screen__group-head">
                <span className="library-screen__swatch" style={{ background: meta.hex }} />
                {meta.label}({items.length})
              </div>
              <div className="library-screen__shelf">
                {items.map((file, i) => (
                  <Book
                    key={file.source}
                    title={getFileTitle(file.source, file.headings)}
                    sourceCategory={file.sourceCategory}
                    size="sm"
                    onClick={() => handleOpenFile(file, getFileCallNo(i, file.sourceCategory))}
                  />
                ))}
              </div>
            </div>
          );
        })}
      </div>

      {selected && (
        <FileModal
          file={selected}
          callNo={selectedCallNo}
          title={getFileTitle(selected.source, selected.headings)}
          onClose={() => setSelected(null)}
          onOpenSource={handleOpenSource}
        />
      )}

      {selected && sourceOpen && (
        <SourceDocumentModal
          title={selected.source}
          heading={getFileTitle(selected.source, selected.headings)}
          content={content}
          error={sourceError}
          onClose={() => setSourceOpen(false)}
        />
      )}
    </div>
  );
}

export default LibraryScreen;
