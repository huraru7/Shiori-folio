import { useEffect, useMemo, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";
import { api } from "../../api/tauri";
import ArticleDetail from "./ArticleDetail";
import FolderTree from "./FolderTree";
import {
  buildFileTree,
  findTreeFolder,
  formatMtime,
  getCategoryMeta,
} from "../../lib/library";
import type { TreeFolder } from "../../lib/library";
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

// パンくず・ツリーのラベル表示。トップレベル(00-inbox等)は詩織の色分け
// ラベルを、それ以外(project名・kind名)はフォルダ名をそのまま使う。
function folderLabel(name: string, depth: number): string {
  return depth === 0 ? getCategoryMeta(name).label : name;
}

// テーブルの列幅比率(%)。「名前」「更新日時」の境界をドラッグすると、
// テーブル全体の幅は変えず、隣り合う列同士で幅を融通し合う(Windows
// エクスプローラーの列リサイズと同じ挙動。2026-09-16、px単位で列を伸ばすと
// テーブル全体がコンテナ幅を超えて崩れる不具合があり比率方式に直した)。
// 「種類」列はname/mtimeの残り(100 - name - mtime)を自動で埋める。
interface ColWidths {
  name: number;
  mtime: number;
}
const DEFAULT_COL_WIDTHS: ColWidths = { name: 50, mtime: 20 };
const MIN_PERCENT = 12;

function clampPercent(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

// <th>の右端にドラッグハンドルを持つ、リサイズ可能な列ヘッダー。ドラッグ量を
// テーブル全体の幅に対する%に換算し、onDragへ差分(%)を渡す(実際にどの列と
// 幅を融通するかは呼び出し側のonDrag実装が決める)。
function ResizableTh({
  label,
  widthPercent,
  onDrag,
}: {
  label: string;
  widthPercent: number;
  onDrag: (deltaPercent: number) => void;
}) {
  const handleMouseDown = (e: ReactMouseEvent<HTMLSpanElement>) => {
    e.preventDefault();
    const startX = e.clientX;
    const table = e.currentTarget.closest("table");
    const tableWidth = table?.getBoundingClientRect().width || 1;
    const handleMouseMove = (moveEvent: MouseEvent) => {
      onDrag(((moveEvent.clientX - startX) / tableWidth) * 100);
    };
    const handleMouseUp = () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  };

  return (
    <th style={{ width: `${widthPercent}%` }}>
      {label}
      {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions */}
      <span className="library-browse-screen__col-resizer" onMouseDown={handleMouseDown} />
    </th>
  );
}

// 列ヘッダー3列分(名前・更新日時・種類)。境界のドラッグで隣接列と幅を
// 融通し合う比率調整ロジックをここに集約し、2つのテーブルで共有する。
function TableHead({
  colWidths,
  onChange,
}: {
  colWidths: ColWidths;
  onChange: (next: ColWidths) => void;
}) {
  const kindPercent = 100 - colWidths.name - colWidths.mtime;
  return (
    <thead>
      <tr>
        <ResizableTh
          label="名前"
          widthPercent={colWidths.name}
          onDrag={(delta) => {
            const maxName = 100 - colWidths.mtime - MIN_PERCENT;
            const newName = clampPercent(colWidths.name + delta, MIN_PERCENT, maxName);
            const actualDelta = newName - colWidths.name;
            onChange({
              name: newName,
              mtime: clampPercent(colWidths.mtime - actualDelta, MIN_PERCENT, 100 - MIN_PERCENT * 2),
            });
          }}
        />
        <ResizableTh
          label="更新日時"
          widthPercent={colWidths.mtime}
          onDrag={(delta) => {
            const maxMtime = 100 - colWidths.name - MIN_PERCENT;
            onChange({
              ...colWidths,
              mtime: clampPercent(colWidths.mtime + delta, MIN_PERCENT, maxMtime),
            });
          }}
        />
        <th style={{ width: `${kindPercent}%` }}>種類</th>
      </tr>
    </thead>
  );
}

// 全件閲覧ウィンドウ(詩織Ver3.1、2026-09-16 UI刷新)。以前は「本棚」の
// メタファーで装丁カードを並べていたが、実データが増えるにつれ「先頭の
// 実見出し(多くは"背景")」がそのまま表紙タイトルになり同じ見た目の本が
// 並んでしまう問題が発覚した。フォルダ階層をそのまま辿れるエクスプローラー
// 風のツリー+テーブル構成に刷新し、frontmatterの本来のtitleを一覧に出す
// ようにした。起動時の自動ロード・接続断リトライのロジックは踏襲する。
function LibraryBrowseScreen() {
  const [files, setFiles] = useState<LibraryFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retryAttempt, setRetryAttempt] = useState(0);
  const [manualRetryKey, setManualRetryKey] = useState(0);
  const [query, setQuery] = useState("");
  const [selectedPath, setSelectedPath] = useState("");
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());
  const [view, setView] = useState<View>({ mode: "list" });
  const [colWidths, setColWidths] = useState<ColWidths>(DEFAULT_COL_WIDTHS);

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

  // ツリーは常に全件から構築する(絞り込み中もフォルダ構造自体は変わらない)。
  const tree = useMemo(() => buildFileTree(files), [files]);

  // 意味検索(LibraryScreenのsearch_library)とは別の、ファイル名・見出し・
  // タイトルの単純な部分一致による簡易検索(UI改善4-2節を踏襲)。
  const normalizedQuery = query.trim().toLowerCase();
  const searchResults = useMemo(() => {
    if (!normalizedQuery) return null;
    return files.filter(
      (f) =>
        f.title.toLowerCase().includes(normalizedQuery) ||
        f.source.toLowerCase().includes(normalizedQuery) ||
        f.headings.some((h) => h.toLowerCase().includes(normalizedQuery)),
    );
  }, [files, normalizedQuery]);

  const currentFolder: TreeFolder | null = useMemo(
    () => findTreeFolder(tree, selectedPath),
    [tree, selectedPath],
  );
  const breadcrumbParts = selectedPath ? selectedPath.split("/").filter(Boolean) : [];

  const toggleExpanded = (path: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const selectFolder = (path: string) => {
    setSelectedPath(path);
    // 選択したフォルダまでの経路を自動展開する(クリックしたのに閉じたまま
    // に見える違和感を避けるため)。
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      const parts = path.split("/").filter(Boolean);
      let acc = "";
      for (const part of parts) {
        acc = acc ? `${acc}/${part}` : part;
        next.add(acc);
      }
      return next;
    });
  };

  const openFile = (file: LibraryFile, callNo: string) =>
    setView({ mode: "detail", source: file.source, sourceCategory: file.sourceCategory, callNo });

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

  const subFolders = currentFolder
    ? [...currentFolder.folders.values()].sort((a, b) => a.name.localeCompare(b.name))
    : [];
  const currentFiles = currentFolder
    ? [...currentFolder.files].sort((a, b) => a.title.localeCompare(b.title))
    : [];

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

      {loading && (
        <p className="library-browse-screen__status">
          蔵書を読み込んでいます…
          {retryAttempt > 0 && `(再試行 ${retryAttempt}/${MAX_AUTO_RETRIES})`}
        </p>
      )}
      {error && !loading && (
        <div className="library-browse-screen__status library-browse-screen__status--error">
          <p>{error}</p>
          <button type="button" className="library-browse-screen__retry-btn" onClick={handleManualRetry}>
            再試行
          </button>
        </div>
      )}

      {!loading && !error && (
        <div className="library-browse-screen__explorer">
          <div className="library-browse-screen__tree">
            <FolderTree
              root={tree}
              selectedPath={selectedPath}
              expandedPaths={expandedPaths}
              onSelect={selectFolder}
              onToggle={toggleExpanded}
            />
          </div>

          <div className="library-browse-screen__main">
            {searchResults ? (
              <>
                <div className="library-browse-screen__breadcrumb">
                  検索結果「{query}」({searchResults.length}件)
                </div>
                <FileTable
                  files={searchResults}
                  onOpen={openFile}
                  colWidths={colWidths}
                  onColWidthsChange={setColWidths}
                />
              </>
            ) : (
              <>
                <div className="library-browse-screen__breadcrumb">
                  <button type="button" onClick={() => selectFolder("")}>
                    library
                  </button>
                  {breadcrumbParts.map((part, i) => {
                    const path = breadcrumbParts.slice(0, i + 1).join("/");
                    return (
                      <span key={path}>
                        <span className="library-browse-screen__breadcrumb-sep">/</span>
                        <button type="button" onClick={() => selectFolder(path)}>
                          {folderLabel(part, i)}
                        </button>
                      </span>
                    );
                  })}
                </div>
                {!currentFolder && (
                  <p className="library-browse-screen__status">このフォルダは見つかりませんでした。</p>
                )}
                {currentFolder && subFolders.length === 0 && currentFiles.length === 0 && (
                  <p className="library-browse-screen__status">このフォルダは空です。</p>
                )}
                {currentFolder && (subFolders.length > 0 || currentFiles.length > 0) && (
                  <FolderContentsTable
                    depth={breadcrumbParts.length}
                    folders={subFolders}
                    files={currentFiles}
                    onOpenFolder={selectFolder}
                    onOpenFile={openFile}
                    colWidths={colWidths}
                    onColWidthsChange={setColWidths}
                  />
                )}
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// フォルダ選択時の一覧(サブフォルダ+ファイルを1つのテーブルに並べる)。
function FolderContentsTable({
  depth,
  folders,
  files,
  onOpenFolder,
  onOpenFile,
  colWidths,
  onColWidthsChange,
}: {
  depth: number;
  folders: TreeFolder[];
  files: LibraryFile[];
  onOpenFolder: (path: string) => void;
  onOpenFile: (file: LibraryFile, callNo: string) => void;
  colWidths: ColWidths;
  onColWidthsChange: (next: ColWidths) => void;
}) {
  return (
    <div className="library-browse-screen__table-wrap">
      <table className="library-browse-screen__table">
        <TableHead colWidths={colWidths} onChange={onColWidthsChange} />
        <tbody>
          {folders.map((folder) => (
            <tr
              key={folder.path}
              className="library-browse-screen__row"
              onClick={() => onOpenFolder(folder.path)}
            >
              <td>
                <span className="library-browse-screen__icon">📁</span>
                {folderLabel(folder.name, depth)}
              </td>
              <td className="library-browse-screen__dim">—</td>
              <td className="library-browse-screen__dim">フォルダ</td>
            </tr>
          ))}
          {files.map((file, i) => (
            <tr
              key={file.source}
              className="library-browse-screen__row"
              onClick={() => onOpenFile(file, `${getCategoryMeta(file.sourceCategory).abbr}-${String(i + 1).padStart(2, "0")}`)}
            >
              <td>
                <span className="library-browse-screen__icon">📄</span>
                {file.title || file.source.replace(/\.md$/i, "")}
              </td>
              <td className="library-browse-screen__dim">{formatMtime(file.mtime) || "—"}</td>
              <td className="library-browse-screen__dim">記事</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// 絞り込み検索結果(フラット表示、フォルダ階層に依存しない)。
function FileTable({
  files,
  onOpen,
  colWidths,
  onColWidthsChange,
}: {
  files: LibraryFile[];
  onOpen: (file: LibraryFile, callNo: string) => void;
  colWidths: ColWidths;
  onColWidthsChange: (next: ColWidths) => void;
}) {
  return (
    <div className="library-browse-screen__table-wrap">
      <table className="library-browse-screen__table">
        <TableHead colWidths={colWidths} onChange={onColWidthsChange} />
        <tbody>
          {files.map((file, i) => (
            <tr
              key={file.source}
              className="library-browse-screen__row"
              onClick={() => onOpen(file, `${getCategoryMeta(file.sourceCategory).abbr}-${String(i + 1).padStart(2, "0")}`)}
            >
              <td>
                <span className="library-browse-screen__icon">📄</span>
                {file.title || file.source.replace(/\.md$/i, "")}
              </td>
              <td className="library-browse-screen__dim">{formatMtime(file.mtime) || "—"}</td>
              <td className="library-browse-screen__dim">{getCategoryMeta(file.sourceCategory).label}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default LibraryBrowseScreen;
