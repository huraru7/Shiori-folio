// 計算機の式の読み取りを、Nodeで画面と同じ決めた式で確かめる。 node --test extensions/calculator/calc.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

// 画面と同じファイルを、そのまま読み込む
const ctx = vm.createContext({});
for (const file of ["calc.js", "selftest-cases.js"]) {
  vm.runInContext(readFileSync(new URL(file, import.meta.url), "utf8"), ctx);
}

for (const c of ctx.CALC_CASES) {
  test(c.label, () => {
    const [ok, detail] = ctx.runCalcCase(ctx.ShioriCalc, c);
    assert.ok(ok, detail);
  });
}
