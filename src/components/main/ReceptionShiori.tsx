import { useEffect, useState, type CSSProperties } from "react";
import { EXT_ORIGIN } from "../extensions/ExtensionFrame";
import "./ReceptionShiori.css";

// ペットと同じ絵(1コマ128×192)とコマの速さ(pet/mod.rsのFPS)。
const FRAME_W = 128;
const FPS = 8;
// ホームでの大きさ(ドット絵をぼかさず1.25倍にする)
const SCALE = 1.25;
// data/pet/の差し替えも効くよう、ペットと同じ専用のURLから読む
const READ_URL = `${EXT_ORIGIN}/_shiori/pet/read.png`;

// ホームの受付で、カウンターの奥で本を読む詩織(2026-10-26)。動きは読書だけ(ふらる)。
// コマ数は絵の幅から求め、コマ送りはCSSのsteps()に任せる(ホームが隠れている間は止まる)。
function ReceptionShiori() {
  const [frames, setFrames] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    const img = new Image();
    img.onload = () => {
      if (!cancelled) setFrames(Math.max(1, Math.round(img.naturalWidth / FRAME_W)));
    };
    img.src = READ_URL;
    return () => {
      cancelled = true;
    };
  }, []);

  const style = {
    "--shiori-frames": frames ?? 1,
    "--shiori-scale": SCALE,
    "--shiori-duration": `${(frames ?? 1) / FPS}s`,
    backgroundImage: frames ? `url(${READ_URL})` : undefined,
  } as CSSProperties;

  return (
    <div className="reception-shiori" aria-hidden="true">
      <div className="reception-shiori__glow" />
      <div className="reception-shiori__sprite" style={style} />
    </div>
  );
}

export default ReceptionShiori;
