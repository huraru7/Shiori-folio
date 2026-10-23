// タイマーの読み取りを、Nodeで画面と同じ決めた確かめで試す。 node --test extensions/timer/timeparse.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

// 画面と同じファイルを、そのまま読み込む
const ctx = vm.createContext({});
for (const file of ["timeparse.js", "selftest-cases.js"]) {
  vm.runInContext(readFileSync(new URL(file, import.meta.url), "utf8"), ctx);
}

for (const c of ctx.TIMER_CASES) {
  test(c.label, () => {
    const [ok, detail] = ctx.runTimerCase(ctx.ShioriTime, c);
    assert.ok(ok, detail);
  });
}

test("表示の形", () => {
  assert.equal(ctx.ShioriTime.formatLeft(299_001), "5:00");
  assert.equal(ctx.ShioriTime.formatLeft(3_909_000), "1:05:09");
  assert.equal(ctx.ShioriTime.formatElapsed(123_456), "2:03.4");
  assert.equal(ctx.ShioriTime.formatElapsed(3_723_400), "1:02:03.4");
});
