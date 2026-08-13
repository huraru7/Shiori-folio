import { useEffect, useState } from "react";
import shioriMark from "../../assets/logo/shiori-mark-startup.png";
import { useShioriStore } from "../../store/useShioriStore";
import "./StartupScreen.css";

interface Props {
  onFinished: () => void;
}

// 起動画面(2026-08-13新規実装)。当初はロゴの色づきを固定2.9秒のCSSアニメーション
// だけで演出していたが、実際のバックエンド起動(LLM/埋め込み/RAG検索サーバーの
// 起動〜モデル読み込み完了)には数秒〜十数秒かかるため、演出と実態が大きく
// ずれていた(同日、起動画面のタイミング問題の指示書で発覚)。
// ensureBackendServicesStarted()の完了を実際に待ち受けるよう修正: 完了するまでは
// 色づきを漸近的に(ASYMPTOTE_PERCENTの手前で足踏みするよう)進め続け、完了を
// 検知したら短時間で残りを埋めてから少し間を置いてフェードアウトする。
const ASYMPTOTE_PERCENT = 90;
const TIME_CONSTANT_MS = 4000;
const CATCH_UP_MS = 400;
const HOLD_MS = 400;
const FADE_MS = 500;

function StartupScreen({ onFinished }: Props) {
  const [percent, setPercent] = useState(0);
  const [ready, setReady] = useState(false);
  const [fadingOut, setFadingOut] = useState(false);
  // Rust側(start_backend_services_impl)が各段階の開始時に発行するshiori:startup-stage
  // イベントを、ストア側(useShioriStore.ensureBackendServicesStarted)でリスナー登録→
  // 起動呼び出しの順を保証した上で購読している(取りこぼし防止)。ここでは購読結果の
  // stateをそのまま表示するだけ。
  const stageLabel = useShioriStore((s) => s.startupStageLabel);

  // バックエンド起動の完了を実際に待ち受ける。App.tsx側でも同じ関数を呼んでいるが、
  // モジュールスコープのPromiseで多重呼び出しを防いでいるため、実際のinvoke()は
  // 1回しか発生しない(どちらが先に呼んでも同じPromiseを共有する)。
  useEffect(() => {
    let cancelled = false;
    useShioriStore
      .getState()
      .ensureBackendServicesStarted()
      .catch(() => {
        // 起動に失敗しても起動画面に固まらないよう、完了扱いにして先へ進む
      })
      .finally(() => {
        if (!cancelled) setReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 完了通知が来るまで、色づきをASYMPTOTE_PERCENT手前まで漸近的に進める
  // (指数関数的に鈍化するので、実際の起動が長引いても唐突に止まって見えない)。
  useEffect(() => {
    if (ready) return;
    let rafId: number;
    const start = performance.now();
    const tick = (now: number) => {
      const elapsed = now - start;
      setPercent(ASYMPTOTE_PERCENT * (1 - Math.exp(-elapsed / TIME_CONSTANT_MS)));
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [ready]);

  // 完了検知後: 残りを一気に埋め、少し間を置いてフェードアウトする
  useEffect(() => {
    if (!ready) return;
    setPercent(100);
    const fadeTimer = setTimeout(() => setFadingOut(true), CATCH_UP_MS + HOLD_MS);
    const doneTimer = setTimeout(onFinished, CATCH_UP_MS + HOLD_MS + FADE_MS);
    return () => {
      clearTimeout(fadeTimer);
      clearTimeout(doneTimer);
    };
  }, [ready, onFinished]);

  return (
    <div className={`startup-screen${fadingOut ? " startup-screen--fade-out" : ""}`}>
      <div className="startup-screen__mark-wrap">
        <img
          src={shioriMark}
          className="startup-screen__mark startup-screen__mark--gray"
          alt=""
          aria-hidden="true"
        />
        <img
          src={shioriMark}
          className={`startup-screen__mark startup-screen__mark--color${ready ? " startup-screen__mark--color--catchup" : ""}`}
          alt="詩織"
          style={{ clipPath: `inset(${100 - percent}% 0 0 0)` }}
        />
      </div>
      <div className="startup-screen__stage-label">{stageLabel}</div>
    </div>
  );
}

export default StartupScreen;
