import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/tauri";
import SourceDocumentModal from "../panels/SourceDocumentModal";
import Book from "./Book";
import FileModal from "./FileModal";
import { CATEGORY_ORDER, getCategoryMeta, getFileCallNo, getFileTitle } from "../../lib/library";
import { useShioriStore } from "../../store/useShioriStore";
import type { LibraryFile } from "../../types";
import "./LibraryScreen.css";

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
  // 失敗し、リトライ手段が無いためそのまま「蔵書が読み込めない」状態に固まって
  // しまう不具合があった(2026-08-14、外付けSSD運用の実機確認で発覚)。
  // ensureBackendServicesStarted()はモジュールスコープの共有Promiseのため、
  // ここで待ち受けても起動処理自体が重複することはない(App.tsx/StartupScreenの
  // 呼び出しと同じPromiseに相乗りするだけ)。他サービス(LLM/embedding)の起動
  // 失敗はここでは無視する(RAG自体は起動できている可能性があるため)。
  useEffect(() => {
    let cancelled = false;
    useShioriStore
      .getState()
      .ensureBackendServicesStarted()
      .catch(() => undefined)
      .then(() => api.listAllKnowledge())
      .then((r) => {
        if (!cancelled) setFiles(r);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
        {loading && <p className="library-screen__status">蔵書を読み込んでいます…</p>}
        {error && <p className="library-screen__status library-screen__status--error">{error}</p>}
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
