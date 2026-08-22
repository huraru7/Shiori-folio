import { useEffect, useState } from "react";
import { marked } from "marked";
import { api } from "../../api/tauri";
import { getCategoryMeta, hexToRgba } from "../../lib/library";
import type { SourceFrontmatter } from "../../types";
import "./ArticleDetail.css";

interface Props {
  source: string;
  sourceCategory: string;
  onBack: () => void;
  // 全件閲覧画面(棚の位置に基づく番号)から開いた場合のみ渡される。検索結果
  // から開いた場合はスコア順であり棚の位置と無関係なため省略される
  // (詩織Ver3.0、UI改善4-2節「分類番号は継続する」)。
  callNo?: string;
}

// frontmatter部分(先頭の---〜---)を取り除いた本文だけをレンダリングする。
// get_source_documentはfrontmatter込みの生Markdown全体を返すため、ここで
// 除去しないとYAML部分がそのまま段落としてレンダリングされてしまう。
function stripFrontmatter(content: string): string {
  const match = content.match(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/);
  return match ? content.slice(match[0].length) : content;
}

// 記事詳細ビュー(詩織Ver3.0、UI改善4-2節)。以前はFileModalとして見出し一覧
// だけを見せるモーダルだったが、今回から本文(markdown)そのものをレンダリング
// して読めるように変更した(案B採用)。人間がUI上で直接本を開いて読む経路の
// 変更であり、会話中にLLMが内容を合成しない「司書モデル」の原則とは矛盾しない
// (原則が適用されるのはAI側の会話応答経路のみ)。検索結果一覧とはウィンドウ内
// 完結の「戻る」操作で行き来する(モーダルの重ね表示ではなく、ビューの切り替え)。
function ArticleDetail({ source, sourceCategory, onBack, callNo }: Props) {
  const [content, setContent] = useState<string | null>(null);
  const [frontmatter, setFrontmatter] = useState<SourceFrontmatter | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setContent(null);
    setFrontmatter(null);
    setError(null);

    Promise.all([
      api.getSourceDocument(sourceCategory, source),
      api.getSourceFrontmatter(sourceCategory, source),
    ])
      .then(([doc, fm]) => {
        if (cancelled) return;
        setContent(doc);
        setFrontmatter(fm);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      });
    return () => {
      cancelled = true;
    };
  }, [source, sourceCategory]);

  const meta = getCategoryMeta(sourceCategory);
  const tagStyle = {
    background: hexToRgba(meta.hex, 0.12),
    borderColor: hexToRgba(meta.hex, 0.3),
    color: meta.hex,
  };
  const html = content ? marked.parse(stripFrontmatter(content), { async: false }) : "";
  const displayTitle = frontmatter?.title || source.replace(/\.md$/i, "");

  return (
    <div className="article-detail">
      <div className="article-detail__header">
        <button className="article-detail__back" onClick={onBack}>
          ← 戻る
        </button>
        <div className="article-detail__header-right">
          {callNo && <span className="article-detail__callno" style={{ color: meta.hex }}>{callNo}</span>}
          <span className="article-detail__tag" style={tagStyle}>
            {meta.label}
          </span>
        </div>
      </div>

      {error && <p className="article-detail__error">{error}</p>}

      {!error && (
        <div className="article-detail__scroll">
          <h2 className="article-detail__title">{displayTitle}</h2>

          {frontmatter && (
            <div className="article-detail__frontmatter">
              {frontmatter.summary && (
                <p className="article-detail__summary">{frontmatter.summary}</p>
              )}
              <div className="article-detail__meta-row">
                <span className="article-detail__status">status: {frontmatter.status}</span>
                {frontmatter.tags.length > 0 && (
                  <span className="article-detail__tags">
                    {frontmatter.tags.map((t) => (
                      <span key={t} className="article-detail__tag-chip">
                        {t}
                      </span>
                    ))}
                  </span>
                )}
              </div>
              {frontmatter.related.length > 0 && (
                <div className="article-detail__related">
                  関連: {frontmatter.related.join(", ")}
                </div>
              )}
            </div>
          )}

          {!content && !error && <p className="article-detail__loading">読み込んでいます…</p>}
          {content && (
            // eslint-disable-next-line react/no-danger
            <div className="article-detail__body" dangerouslySetInnerHTML={{ __html: html }} />
          )}

          <h4 className="article-detail__source-heading">この記録の出どころ</h4>
          <p className="article-detail__source">{source}</p>
        </div>
      )}
    </div>
  );
}

export default ArticleDetail;
