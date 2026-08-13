import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/tauri";
import SourceDocumentModal from "../panels/SourceDocumentModal";
import Book from "./Book";
import ChunkModal from "./ChunkModal";
import { CATEGORY_ORDER, getCategoryMeta } from "../../lib/library";
import type { KnowledgeResult } from "../../types";
import "./LibraryScreen.css";

// 図書館ウィンドウ(2026-08-12、デスクトップ型ウィンドウシステムの本実装3-2)。
// 以前はスタンドアロンの全画面ビューだったが、OsWindow内のコンパクト版に
// 作り替えた(開閉・ドラッグ・リサイズ等はOsWindow側が担うため、ここでは
// 検索バー固定+一覧スクロールの中身だけを持つ)。検索を経由しない蔵書全件を
// 取得し、カテゴリ別・ファイル単位の棚に並べる。ここでは既に棚に並んでいる
// ものを自分の意思で見にいくだけなので、KnowledgePanelのgather演出は発動しない。
function LibraryScreen() {
  const [chunks, setChunks] = useState<KnowledgeResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const [selected, setSelected] = useState<KnowledgeResult | null>(null);
  const [sourceOpen, setSourceOpen] = useState(false);
  const [content, setContent] = useState<string | null>(null);
  const [sourceError, setSourceError] = useState<string | null>(null);

  useEffect(() => {
    api
      .listAllKnowledge()
      .then((r) => setChunks(r))
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
  }, []);

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return chunks;
    return chunks.filter(
      (c) =>
        c.heading.toLowerCase().includes(normalized) ||
        c.text.toLowerCase().includes(normalized) ||
        c.source.toLowerCase().includes(normalized),
    );
  }, [chunks, query]);

  // カテゴリ別の棚の中でも、由来元のファイル(章)ごとに薄い枠でグルーピングする。
  // 大量の本が同色で埋め尽くされて見づらくなるのを防ぐため。
  const grouped = useMemo(() => {
    const byCategory = new Map<string, Map<string, KnowledgeResult[]>>();
    for (const chunk of filtered) {
      const fileMap = byCategory.get(chunk.sourceCategory) ?? new Map<string, KnowledgeResult[]>();
      const list = fileMap.get(chunk.source) ?? [];
      list.push(chunk);
      fileMap.set(chunk.source, list);
      byCategory.set(chunk.sourceCategory, fileMap);
    }
    const orderedKeys = [
      ...CATEGORY_ORDER.filter((c) => byCategory.has(c)),
      ...[...byCategory.keys()].filter((c) => !(CATEGORY_ORDER as readonly string[]).includes(c)),
    ];
    return orderedKeys.map((category) => {
      const fileMap = byCategory.get(category)!;
      const fileGroups = [...fileMap.entries()].map(([source, items]) => ({ source, items }));
      const totalCount = fileGroups.reduce((sum, g) => sum + g.items.length, 0);
      return { category, fileGroups, totalCount };
    });
  }, [filtered]);

  const handleOpenChunk = (result: KnowledgeResult) => {
    setSelected(result);
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

        {grouped.map(({ category, fileGroups, totalCount }) => {
          const meta = getCategoryMeta(category);
          return (
            <div className="library-screen__group" key={category}>
              <div className="library-screen__group-head">
                <span className="library-screen__swatch" style={{ background: meta.hex }} />
                {meta.label}({totalCount})
              </div>
              <div className="library-screen__shelf">
                {fileGroups.map(({ source, items }) => (
                  <div className="library-screen__file-group" key={source} title={source}>
                    {items.map((item) => (
                      <Book key={item.id} result={item} size="sm" onClick={() => handleOpenChunk(item)} />
                    ))}
                  </div>
                ))}
              </div>
            </div>
          );
        })}
      </div>

      {selected && (
        <ChunkModal
          result={selected}
          onClose={() => setSelected(null)}
          onOpenSource={handleOpenSource}
        />
      )}

      {selected && sourceOpen && (
        <SourceDocumentModal
          title={selected.source}
          heading={selected.heading}
          content={content}
          error={sourceError}
          onClose={() => setSourceOpen(false)}
        />
      )}
    </div>
  );
}

export default LibraryScreen;
