import type { CSSProperties } from "react";

interface Props {
  className?: string;
  style?: CSSProperties;
}

// Dockアイコン専用の簡略ロゴ(2026-08-13)。本物のロゴ(栞の波線・色分け)は
// 34px程度まで縮小すると波線が潰れて視認できなくなるため、輪郭形状(縦長の
// 四角＋下端がV字カットされた栞のシルエット)のみを単色塗りで再現したもの。
// ヘッダー・起動画面は引き続き実物のロゴ画像(src/assets/logo)を使用する。
function ShioriMarkGlyph({ className, style }: Props) {
  return (
    <svg viewBox="0 0 100 220" className={className} style={style} xmlns="http://www.w3.org/2000/svg">
      <path d="M0,0 L100,0 L100,220 L50,132 L0,220 Z" fill="#8FA06B" />
      <path d="M88,0 L100,0 L100,132 L88,132 Z" fill="#C68A5E" />
    </svg>
  );
}

export default ShioriMarkGlyph;
