// タイマーの決めた確かめ(画面の自己確認とNodeの確かめで同じものを使う)。
// いまの時刻は 2026-10-23 14:00:00(その地域の時刻)に固定する。
(function (root) {
  "use strict";
  const NOW = new Date(2026, 9, 23, 14, 0, 0, 0).getTime();
  const MIN = 60 * 1000;
  const at = (d, h, m) => new Date(2026, 9, d, h, m, 0, 0).getTime();

  root.TIMER_CASES = [
    { label: "分", text: "30分", at: NOW + 30 * MIN },
    { label: "分後", text: "30分後", at: NOW + 30 * MIN },
    { label: "時間と分", text: "1時間30分", at: NOW + 90 * MIN },
    { label: "時間半", text: "1時間半", at: NOW + 90 * MIN },
    { label: "秒", text: "90秒", at: NOW + 90 * 1000 },
    { label: "コロン3つは長さ", text: "1:30:00", at: NOW + 90 * MIN },
    { label: "数だけは分", text: "5", at: NOW + 5 * MIN },
    { label: "全角", text: "３分", at: NOW + 3 * MIN },
    { label: "時刻(コロン)", text: "13:30", at: at(24, 13, 30) },
    { label: "時刻(半)", text: "13時半", at: at(24, 13, 30) },
    { label: "時刻(この後)", text: "15時", at: at(23, 15, 0) },
    { label: "時刻(分)", text: "14時5分", at: at(23, 14, 5) },
    { label: "いまと同じ時刻は翌日", text: "14:00", at: at(24, 14, 0) },
    { label: "日をまたぐ", text: "0:15", at: at(24, 0, 15) },
    { label: "読めない", text: "そのうち", error: "timer-001" },
    { label: "時刻の数が大きい", text: "25:00", error: "timer-001" },
    { label: "空", text: "", error: "timer-001" },
    { label: "7日より先", text: "200時間", error: "timer-001" },
  ];

  // 1件を試す。[合っているか, 中身]
  root.runTimerCase = function (time, c) {
    try {
      const got = time.parse(c.text, NOW).at;
      if (c.error) return [false, `エラーになるはずが ${new Date(got).toString()}`];
      return [got === c.at, `${new Date(got).toString()} / 期待 ${new Date(c.at).toString()}`];
    } catch (e) {
      if (c.error) return [e.code === c.error, `${e.code}: ${e.message}`];
      return [false, `${e.code}: ${e.message}`];
    }
  };
})(typeof window !== "undefined" ? window : globalThis);
