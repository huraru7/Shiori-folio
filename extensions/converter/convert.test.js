// 変換(converter)のNodeでの確かめ。`node --test extensions/converter/convert.test.js` で走らせる。
// 画面と同じ convert.js と selftest.js を vm で読み、selftest.js の一覧をそのまま試す。
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const context = vm.createContext({});
for (const file of ["convert.js", "selftest.js"]) {
  vm.runInContext(readFileSync(new URL(file, import.meta.url), "utf8"), context, { filename: file });
}
const C = context.ShioriConvert;

for (const t of context.ShioriConvertTests) {
  test(t.label, () => {
    const [ok, detail] = t.run(C);
    assert.ok(ok, `期待: ${t.expect} / 結果: ${detail}`);
  });
}
