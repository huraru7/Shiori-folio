import { useEffect, useRef } from "react";
import { marked } from "marked";
import "./SourceDocumentModal.css";

interface Props {
  title: string;
  heading: string;
  content: string | null;
  error: string | null;
  onClose: () => void;
}

// KnowledgePanelの参照情報をクリックしたときに、元のMarkdownファイル全文を
// 表示するモーダル(v1.0スコープ機能1)。マークダウンのレンダリングは軽量な
// markedを使い(React用の重いレンダラーは導入しない)、HTML文字列を
// dangerouslySetInnerHTMLで挿入する。該当する見出しへの自動スクロール・
// ハイライトは、レンダリング後にDOMを直接走査して該当する見出し要素を
// 探す方式にしている(markedの出力をそのまま使うため、見出しテキストの
// 前処理は不要)。
function SourceDocumentModal({ title, heading, content, error, onClose }: Props) {
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!content || !bodyRef.current || heading === "(見出しなし)") return;
    const headingElements = bodyRef.current.querySelectorAll("h1, h2, h3, h4, h5, h6");
    for (const el of headingElements) {
      if (el.textContent?.trim() === heading.trim()) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
        el.classList.add("source-document-modal__highlight");
        break;
      }
    }
  }, [content, heading]);

  const html = content ? marked.parse(content, { async: false }) : "";

  return (
    <div className="source-document-modal__backdrop" onClick={onClose}>
      <div className="source-document-modal" onClick={(e) => e.stopPropagation()}>
        <div className="source-document-modal__header">
          <span className="source-document-modal__title">{title}</span>
          <button className="source-document-modal__close" onClick={onClose} aria-label="閉じる">
            ×
          </button>
        </div>
        <div className="source-document-modal__body" ref={bodyRef}>
          {error && <p className="source-document-modal__error">{error}</p>}
          {!error && !content && <p>読み込み中...</p>}
          {!error && content && (
            // eslint-disable-next-line react/no-danger
            <div dangerouslySetInnerHTML={{ __html: html }} />
          )}
        </div>
      </div>
    </div>
  );
}

export default SourceDocumentModal;
