import { useEffect, useMemo, useRef } from "react";
import { useMainViewStore } from "../../store/useMainViewStore";
import { useReduceMotion } from "../../store/useMotionStore";
import "./Effects.css";

// 画面の演出(詩織Ver6.0の段階2)。
// - 水紋: data-rippleの付いた項目・ボタンを押したとき、押した場所から静かに広がる輪。
//   画面を切り替えたときは、画面の真ん中から大きく薄い輪を1つ出す
// - 星: 背景の端に、小さな点がまばらに瞬く(画面の中身の後ろにあり、読みやすさを邪魔しない)
// 「動きを抑える」が入っているときは、どちらも出さない。

// 強さと頻度(ふらるに見てもらいながら決める。段階2の未確定事項)
const RIPPLE_MS = 600;
const STAR_COUNT = 18;
// 端からどこまでを「端」とみなすか(画面の幅・高さに対する割合)
const STAR_EDGE = 0.14;

function spawnRipple(layer: HTMLElement, x: number, y: number, size: number, className: string) {
  const ring = document.createElement("span");
  ring.className = className;
  ring.style.left = `${x - size / 2}px`;
  ring.style.top = `${y - size / 2}px`;
  ring.style.width = `${size}px`;
  ring.style.height = `${size}px`;
  ring.style.animationDuration = `${RIPPLE_MS}ms`;
  ring.addEventListener("animationend", () => ring.remove());
  layer.appendChild(ring);
}

// 星の位置。毎回の起動で同じにならないよう乱数で決め、端の帯の中だけに置く。
function makeStars() {
  return Array.from({ length: STAR_COUNT }, (_, i) => {
    const side = i % 4;
    const along = Math.random() * 100;
    const into = Math.random() * STAR_EDGE * 100;
    const pos =
      side === 0
        ? { left: along, top: into }
        : side === 1
          ? { left: 100 - into, top: along }
          : side === 2
            ? { left: along, top: 100 - into }
            : { left: into, top: along };
    return {
      ...pos,
      size: Math.random() < 0.25 ? 3 : 2,
      // 1つの星が光るのは周期の1割ほど。周期と始まりをばらして、同時に光るのを数個までにする
      duration: 9 + Math.random() * 10,
      delay: -Math.random() * 20,
    };
  });
}

function Effects() {
  const reduce = useReduceMotion();
  const layerRef = useRef<HTMLDivElement>(null);
  const active = useMainViewStore((s) => s.active);
  const stars = useMemo(makeStars, []);
  const first = useRef(true);

  useEffect(() => {
    if (reduce) return;
    const onDown = (e: PointerEvent) => {
      const target = (e.target as HTMLElement | null)?.closest("[data-ripple]");
      if (!target || !layerRef.current) return;
      spawnRipple(layerRef.current, e.clientX, e.clientY, 120, "effects__ripple");
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [reduce]);

  useEffect(() => {
    // 起動したときの最初の画面では出さない
    if (first.current) {
      first.current = false;
      return;
    }
    if (reduce || !layerRef.current) return;
    const pane = document.querySelector<HTMLElement>(`[data-view-id="${CSS.escape(active)}"]`);
    const rect = pane?.getBoundingClientRect();
    if (!rect || rect.width === 0) return;
    spawnRipple(
      layerRef.current,
      rect.left + rect.width / 2,
      rect.top + rect.height / 2,
      Math.max(rect.width, rect.height) * 0.9,
      "effects__ripple effects__ripple--wide",
    );
  }, [active, reduce]);

  return (
    <>
      {!reduce && (
        <div className="effects__stars" aria-hidden="true" data-star-count={stars.length}>
          {stars.map((s, i) => (
            <span
              key={i}
              className="effects__star"
              style={{
                left: `${s.left}%`,
                top: `${s.top}%`,
                width: `${s.size}px`,
                height: `${s.size}px`,
                animationDuration: `${s.duration}s`,
                animationDelay: `${s.delay}s`,
              }}
            />
          ))}
        </div>
      )}
      <div ref={layerRef} className="effects__ripples" aria-hidden="true" data-reduce-motion={reduce} />
    </>
  );
}

export default Effects;
