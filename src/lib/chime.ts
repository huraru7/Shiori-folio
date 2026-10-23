import type { NoticeKind } from "../types";

// 知らせの音(詩織Ver6.0の段階3)。ファイルを持たず、画面の中で短い音を作る。
// 種類ごとに高さを変え、どの知らせか耳で分かるようにする。

// 種類ごとの音の高さ(Hz)。2つの音を続けて鳴らす
const NOTES: Record<NoticeKind, [number, number]> = {
  timer: [880, 1320],
  claude: [660, 880],
  pending: [587, 784],
  extension: [740, 988],
};

let context: AudioContext | null = null;

export function playChime(kind: NoticeKind): void {
  try {
    context ??= new AudioContext();
    const ctx = context;
    if (ctx.state === "suspended") ctx.resume().catch(() => undefined);
    NOTES[kind].forEach((freq, i) => {
      const start = ctx.currentTime + i * 0.18;
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = freq;
      // 静かに立ち上がり、ゆっくり消える
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.exponentialRampToValueAtTime(0.18, start + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.6);
      osc.connect(gain).connect(ctx.destination);
      osc.start(start);
      osc.stop(start + 0.62);
    });
  } catch {
    // 音が出せなくても、札は出す
  }
}
