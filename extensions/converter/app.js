// 変換(converter)の画面。計算は convert.js(ShioriConvert)、確かめの一覧は selftest.js(ShioriConvertTests)。
// 詩織への頼みごとは clipboard.write と storage だけ(説明書の uses)。log.write は誰でも使える。
const C = window.ShioriConvert;

// 種類ごとの、はじめに選んでおく元の単位
const DEFAULT_FROM = {
  length: "m",
  mass: "g",
  temperature: "C",
  data: "MB",
  time: "min",
  speed: "kmh",
  area: "m2",
  volume: "L",
};
const STORAGE_KEY = "lastCategory";

const $ = (id) => document.getElementById(id);
const els = {
  tabUnit: $("tab-unit"),
  tabRadix: $("tab-radix"),
  paneUnit: $("pane-unit"),
  paneRadix: $("pane-radix"),
  categories: $("categories"),
  value: $("unit-value"),
  from: $("unit-from"),
  unitError: $("unit-error"),
  unitResults: $("unit-results"),
  radixRows: $("radix-rows"),
  radixError: $("radix-error"),
  status: $("status"),
};

let categoryId = "length";
// 種類ごとに、最後に選んだ元の単位(画面を開いている間だけ)
const fromByCategory = { ...DEFAULT_FROM };

// ---- 知らせの一行(コピーの結果など) ----
let statusTimer = 0;
function showStatus(text, kind) {
  els.status.textContent = text;
  els.status.className = `status ${kind ?? ""}`;
  clearTimeout(statusTimer);
  statusTimer = setTimeout(() => {
    els.status.textContent = "";
  }, 4000);
}

// ---- コピー ----
async function copy(text) {
  if (!text) return;
  try {
    await shiori.call("clipboard.write", { text });
    showStatus(`写した: ${text}`, "ok");
  } catch (e) {
    // 本体の窓口がまだのときはEXT-012が返る。控えめに出すだけにする
    if (e.code === "EXT-012") showStatus("コピーは、詩織の側がまだ使えない(EXT-012)", "quiet");
    else showStatus(`写せなかった(${e.code ?? "?"}): ${e.message}`, "ng");
  }
}

function copyButton(getText, label) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "copy";
  b.textContent = "写す";
  b.setAttribute("aria-label", `${label}を写す`);
  b.addEventListener("click", () => copy(getText()));
  return b;
}

// ---- タブ ----
function selectTab(tab) {
  const unit = tab === "unit";
  els.tabUnit.setAttribute("aria-selected", String(unit));
  els.tabRadix.setAttribute("aria-selected", String(!unit));
  els.paneUnit.hidden = !unit;
  els.paneRadix.hidden = unit;
}
els.tabUnit.addEventListener("click", () => selectTab("unit"));
els.tabRadix.addEventListener("click", () => selectTab("radix"));

// ---- 単位 ----
function renderCategories() {
  els.categories.textContent = "";
  for (const c of C.CATEGORIES) {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = c.label;
    b.dataset.id = c.id;
    b.setAttribute("aria-pressed", String(c.id === categoryId));
    b.addEventListener("click", () => selectCategory(c.id, true));
    els.categories.appendChild(b);
  }
}

function selectCategory(id, remember) {
  if (!C.CATEGORIES.some((c) => c.id === id)) return;
  categoryId = id;
  for (const b of els.categories.children) b.setAttribute("aria-pressed", String(b.dataset.id === id));
  const category = C.findCategory(id);
  els.from.textContent = "";
  for (const u of category.units) {
    const o = document.createElement("option");
    o.value = u.id;
    o.textContent = u.label;
    els.from.appendChild(o);
  }
  els.from.value = fromByCategory[id];
  renderUnitResults();
  if (remember) shiori.call("storage.set", { key: STORAGE_KEY, value: id }).catch(() => undefined);
}

function renderUnitResults() {
  els.unitResults.textContent = "";
  els.unitError.hidden = true;
  let value;
  try {
    value = C.parseNumber(els.value.value);
  } catch (e) {
    els.unitError.textContent = `${e.message}(${e.code})`;
    els.unitError.hidden = false;
    els.value.setAttribute("aria-invalid", "true");
    return;
  }
  els.value.removeAttribute("aria-invalid");
  if (value === null) return;
  const fromId = els.from.value;
  for (const row of C.convertAll(categoryId, fromId, value)) {
    const text = C.formatNumber(row.value);
    const li = document.createElement("li");
    if (row.id === fromId) li.className = "source-row";
    const num = document.createElement("span");
    num.className = "num";
    num.textContent = text;
    const unit = document.createElement("span");
    unit.className = "unit";
    unit.textContent = row.label;
    li.append(num, unit, copyButton(() => text, `${text} ${row.label}`));
    els.unitResults.appendChild(li);
  }
}

els.value.addEventListener("input", renderUnitResults);
els.from.addEventListener("change", () => {
  fromByCategory[categoryId] = els.from.value;
  renderUnitResults();
});

// ---- 基数 ----
const radixInputs = new Map();

function renderRadixRows() {
  for (const r of C.RADIXES) {
    const li = document.createElement("li");
    const label = document.createElement("label");
    label.className = "unit";
    label.textContent = r.label;
    label.htmlFor = `radix-${r.radix}`;
    const input = document.createElement("input");
    input.type = "text";
    input.id = `radix-${r.radix}`;
    input.autocomplete = "off";
    input.spellcheck = false;
    input.inputMode = r.radix === 16 ? "text" : "numeric";
    input.addEventListener("input", () => onRadixInput(r.radix));
    radixInputs.set(r.radix, input);
    li.append(label, input, copyButton(() => input.value.trim(), r.label));
    els.radixRows.appendChild(li);
  }
}

function onRadixInput(radix) {
  const input = radixInputs.get(radix);
  els.radixError.hidden = true;
  for (const i of radixInputs.values()) i.removeAttribute("aria-invalid");
  let out;
  try {
    out = C.convertRadixAll(input.value, radix);
  } catch (e) {
    // 打っている欄はそのまま残し、ほかの欄も変えない
    els.radixError.textContent = `${e.message}(${e.code})`;
    els.radixError.hidden = false;
    input.setAttribute("aria-invalid", "true");
    return;
  }
  for (const [r, i] of radixInputs) {
    if (r === radix) continue;
    i.value = out ? out[r] : "";
  }
}

// ---- 自己確認 ----
// 画面だけで確かめるもの(Nodeでは確かめられない分)。selftest.jsの一覧のあとに走らせる
const UI_TESTS = [
  {
    label: "画面: 10進の欄に255と打つと、ほかの欄が変わる",
    expect: "2進=11111111、8進=377、16進=FF",
    run: () => {
      const input = radixInputs.get(10);
      input.value = "255";
      input.dispatchEvent(new Event("input"));
      const got = [2, 8, 16].map((r) => radixInputs.get(r).value);
      for (const i of radixInputs.values()) i.value = "";
      return [got.join(",") === "11111111,377,FF", got.join(" / ")];
    },
  },
  {
    label: "画面: 長さで1mと入れると、10個の単位が並ぶ",
    expect: "10行",
    run: () => {
      const keep = { categoryId, value: els.value.value, from: fromByCategory.length };
      fromByCategory.length = "m";
      selectCategory("length", false);
      els.value.value = "1";
      renderUnitResults();
      const n = els.unitResults.children.length;
      fromByCategory.length = keep.from;
      els.value.value = keep.value;
      selectCategory(keep.categoryId, false);
      return [n === 10, `${n}行`];
    },
  },
];

async function runSelfTest() {
  const tests = [...window.ShioriConvertTests, ...UI_TESTS];
  let passed = 0;
  for (const t of tests) {
    let ok = false;
    let detail = "";
    try {
      [ok, detail] = t.run(C);
    } catch (e) {
      detail = `試す途中で失敗: ${e}`;
    }
    if (ok) passed += 1;
    // 通ったものはまとめて数だけ、違ったものは1つずつログに書く(54件を全部書くと読みにくい)
    if (!ok) {
      await shiori
        .call("log.write", {
          level: "warn",
          event: "試した",
          fields: { 項目: t.label, 期待: t.expect, 結果: "違う", 中身: String(detail).slice(0, 160) },
        })
        .catch(() => undefined);
    }
  }
  await shiori
    .call("log.write", {
      level: passed === tests.length ? "info" : "warn",
      event: "全部試した",
      fields: { 期待どおり: String(passed), 全部: String(tests.length) },
    })
    .catch(() => undefined);
  return { ok: passed, ng: tests.length - passed };
}

let selfTestRunning = false;
window.addEventListener("message", async (event) => {
  if (event.source !== window.parent || !event.data || event.data.shiori !== 1 || !event.data.selftestRun) return;
  if (selfTestRunning) return;
  selfTestRunning = true;
  window.parent.postMessage({ shiori: 1, selftest: { started: true } }, "*");
  const result = await runSelfTest();
  window.parent.postMessage({ shiori: 1, selftest: result }, "*");
  selfTestRunning = false;
});

// ---- はじめ ----
renderCategories();
selectCategory(categoryId, false);
renderRadixRows();
shiori
  .call("storage.get", { key: STORAGE_KEY })
  .then((saved) => {
    if (typeof saved === "string" && saved !== categoryId) selectCategory(saved, false);
  })
  .catch(() => undefined);
