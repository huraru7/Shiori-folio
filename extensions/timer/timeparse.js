// タイマーの「長さか時刻」の読み取り(詩織Ver6.0の段階3)。画面とNodeの確かめの両方で使う。
// 長さ: 30分・1時間30分・1時間半・90秒・1:30:00・5(数だけは分)。時刻: 13:30・13時半・13時・13時5分。
// 時刻がいまより前(同じを含む)なら、翌日のその時刻にする。予約できるのは7日以内(本体の決まりと同じ)。
(function (root) {
  "use strict";

  const MAX_MS = 7 * 24 * 60 * 60 * 1000;

  function fail(message) {
    const e = new Error(message);
    e.code = "timer-001";
    return e;
  }

  // 全角の数字と記号を半角に、空白を取り、終わりの「後」「に」「まで」を外す
  function normalize(text) {
    return String(text)
      .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0))
      .replace(/[：]/g, ":")
      .replace(/\s+/g, "")
      .replace(/(後|あと|に|まで)$/u, "");
  }

  // 長さ(ミリ秒)。読めなければnull
  function parseDuration(t) {
    let m = t.match(/^(\d+):(\d{1,2}):(\d{1,2})$/);
    if (m) return ((+m[1] * 60 + +m[2]) * 60 + +m[3]) * 1000;
    if (/^\d+$/.test(t)) return +t * 60 * 1000;
    m = t.match(/^(?:(\d+)時間(半)?)?(?:(\d+)分(半)?)?(?:(\d+)秒)?$/u);
    if (!m || !(m[1] || m[3] || m[5])) return null;
    const hours = +(m[1] ?? 0);
    const minutes = +(m[3] ?? 0) + (m[2] ? 30 : 0);
    const seconds = +(m[5] ?? 0) + (m[4] ? 30 : 0);
    return ((hours * 60 + minutes) * 60 + seconds) * 1000;
  }

  // 時刻 [時, 分]。読めなければnull
  function parseClock(t) {
    let m = t.match(/^(\d{1,2}):(\d{2})$/);
    if (!m) m = t.match(/^(\d{1,2})時(?:(半)|(\d{1,2})分)?$/u);
    if (!m) return null;
    const hour = +m[1];
    // コロンの形は m[2] が分、「時」の形は m[2] が「半」か、m[3] が分
    const minute = m[2] === "半" ? 30 : +(m[3] ?? m[2] ?? 0);
    if (hour > 23 || minute > 59) throw fail("時刻の数が大きすぎる");
    return [hour, minute];
  }

  // text と いまの時刻(ミリ秒)から、予約する時刻を決める。{at, kind: "duration"|"clock"}
  function parse(text, now) {
    const t = normalize(text);
    if (!t) throw fail("空");
    // 「1時間」は時刻ではなく長さ。「13時」「13:30」は時刻
    const clock = /時間/u.test(t) || /^\d+:\d+:\d+$/.test(t) ? null : parseClock(t);
    if (clock) {
      const d = new Date(now);
      d.setHours(clock[0], clock[1], 0, 0);
      if (d.getTime() <= now) d.setDate(d.getDate() + 1);
      return { at: d.getTime(), kind: "clock" };
    }
    const ms = parseDuration(t);
    if (ms === null) throw fail("長さか時刻として読めない");
    if (ms <= 0) throw fail("0より長くする");
    if (ms > MAX_MS) throw fail("7日以内にする");
    return { at: now + ms, kind: "duration" };
  }

  // 残りの時間を「1:05:09」「4:59」の形に
  function formatLeft(ms) {
    const total = Math.max(0, Math.ceil(ms / 1000));
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const pad = (n) => String(n).padStart(2, "0");
    return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
  }

  // ストップウォッチの経過を「1:02:03.4」「2:03.4」の形に(10分の1秒まで)
  function formatElapsed(ms) {
    const tenths = Math.floor(Math.max(0, ms) / 100);
    const total = Math.floor(tenths / 10);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    const pad = (n) => String(n).padStart(2, "0");
    return `${h > 0 ? `${h}:${pad(m)}` : m}:${pad(s)}.${tenths % 10}`;
  }

  root.ShioriTime = Object.freeze({ parse, formatLeft, formatElapsed });
})(typeof window !== "undefined" ? window : globalThis);
