import type { CSSProperties } from "react";
import { getBookCategoryClass } from "../../lib/library";
import "./Book.css";

// 本ごとに少しずつ違う方向から飛んでくるように見せるための移動元オフセット
// (--fx/--fy/--fr、Book.cssの@keyframes book-gatherで参照)。CSS変数は
// TypeScript標準の型では表現できないため、Record<string, string>として渡す。
type FlyFromVars = CSSProperties & Record<"--fx" | "--fy" | "--fr", string>;

interface Props {
  // 表紙に表示するタイトル。KnowledgePanel(チャンク単位)ではチャンクの
  // 見出し、LibraryScreen(Phase 7以降、ファイル単位)ではファイルの代表見出し
  // またはファイル名を渡す(呼び出し側の粒度が異なっても見た目は共通のため)。
  title: string;
  sourceCategory: string;
  onClick: () => void;
  // 「司書が持ってくる棚」の集まってくる演出用。gatherDelayMsを指定すると、
  // その分だけ遅れてgatherアニメーションが再生される(本ごとの時間差を出すため)。
  gathering?: boolean;
  gatherDelayMs?: number;
  flyFrom?: { fx: string; fy: string; fr: string };
  // ウィンドウ内表示用のサイズ(2026-08-12、デスクトップ型ウィンドウシステムの
  // 本実装)。md=ナレッジウィンドウ(68×92)、sm=図書館ウィンドウ(52×70)。
  // window-contents-concept.htmlのモックアップに合わせ、タイトルのみを表示する
  // (分類番号・カテゴリラベルはクリック後のモーダルで確認できるため省略)。
  size?: "md" | "sm";
}

// 図書館UI共通の「本」1冊分の見た目。表紙カード方式。
// KnowledgePanel(チャンク単位)・LibraryScreen(Phase 7以降、ファイル単位)で
// 表示粒度は異なるが、見た目のコンポーネントとしては共有する。
function Book({ title, sourceCategory, onClick, gathering, gatherDelayMs, flyFrom, size = "md" }: Props) {
  const categoryClass = getBookCategoryClass(sourceCategory);

  const style: FlyFromVars | undefined = gathering
    ? {
        animationDelay: `${gatherDelayMs ?? 0}ms`,
        "--fx": flyFrom?.fx ?? "0px",
        "--fy": flyFrom?.fy ?? "-60px",
        "--fr": flyFrom?.fr ?? "-12deg",
      }
    : undefined;

  return (
    <div
      className={`book book--${size} ${categoryClass} ${gathering ? "book--gathering" : ""}`}
      style={style}
      onClick={onClick}
      title={title}
    >
      <div className="book__title">{title}</div>
    </div>
  );
}

export default Book;
