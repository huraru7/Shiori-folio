// 計算機の画面(詩織Ver6.0の段階3)。式の読み取りは calc.js、決めた式は selftest-cases.js。
const HISTORY_MAX = 10;
const exprEl = document.getElementById("expr");
const resultEl = document.getElementById("result");
const messageEl = document.getElementById("message");
const angleEl = document.getElementById("angle");
const historyEl = document.getElementById("history");

let angle = "deg";
let lastAnswer; // ans で使う、直前に確定した答え
let history = []; // [{expr, result}] 新しいものが先

function say(text) {
  messageEl.textContent = text;
}

// 打っている途中は答えを薄く出すだけで、エラーは出さない
function preview() {
  try {
    resultEl.textContent = ShioriCalc.format(ShioriCalc.evaluate(exprEl.value, { angle, ans: lastAnswer }));
    resultEl.classList.remove("dim");
  } catch {
    resultEl.classList.add("dim");
  }
}

function equal() {
  const expr = exprEl.value.trim();
  if (!expr) return;
  try {
    const value = ShioriCalc.evaluate(expr, { angle, ans: lastAnswer });
    const shown = ShioriCalc.format(value);
    lastAnswer = value;
    resultEl.textContent = shown;
    resultEl.classList.remove("dim");
    say("");
    addHistory(expr, shown);
  } catch (e) {
    say(`${e.code ?? "calculator-001"}: ${e.message}`);
  }
}

function insert(text) {
  const start = exprEl.selectionStart ?? exprEl.value.length;
  const end = exprEl.selectionEnd ?? start;
  exprEl.value = exprEl.value.slice(0, start) + text + exprEl.value.slice(end);
  exprEl.setSelectionRange(start + text.length, start + text.length);
  exprEl.focus();
  preview();
}

function renderHistory() {
  historyEl.textContent = "";
  for (const item of history) {
    const li = document.createElement("li");
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = `${item.expr} = ${item.result}`;
    btn.addEventListener("click", () => {
      exprEl.value = item.expr;
      exprEl.focus();
      preview();
    });
    li.appendChild(btn);
    historyEl.appendChild(li);
  }
}

function addHistory(expr, result) {
  if (history[0]?.expr === expr && history[0]?.result === result) return;
  history = [{ expr, result }, ...history].slice(0, HISTORY_MAX);
  renderHistory();
  shiori.call("storage.set", { key: "history", value: history }).catch((e) => say(`履歴を保存できない(${e.code})`));
}

function setAngle(next) {
  angle = next;
  angleEl.textContent = angle === "deg" ? "度" : "ラジアン";
  preview();
}

document.querySelectorAll(".keys button").forEach((btn) => {
  btn.type = "button";
  btn.addEventListener("click", () => {
    const act = btn.dataset.act;
    if (act === "equal") equal();
    else if (act === "clear") { exprEl.value = ""; resultEl.textContent = "0"; say(""); exprEl.focus(); }
    else if (act === "back") insertBack();
    else insert(btn.dataset.ins);
  });
});

function insertBack() {
  const start = exprEl.selectionStart ?? exprEl.value.length;
  const end = exprEl.selectionEnd ?? start;
  const from = start === end ? Math.max(0, start - 1) : start;
  exprEl.value = exprEl.value.slice(0, from) + exprEl.value.slice(end);
  exprEl.setSelectionRange(from, from);
  exprEl.focus();
  preview();
}

exprEl.addEventListener("input", preview);
exprEl.addEventListener("keydown", (e) => {
  if (e.key === "Enter") { e.preventDefault(); equal(); }
});

angleEl.addEventListener("click", () => {
  setAngle(angle === "deg" ? "rad" : "deg");
  shiori.call("storage.set", { key: "angle", value: angle }).catch(() => undefined);
});

// 答えのコピー。本体の窓口がまだのとき(EXT-012)は、控えめに出す
document.getElementById("copy").addEventListener("click", async () => {
  try {
    await shiori.call("clipboard.write", { text: resultEl.textContent });
    say("答えをコピーした");
  } catch (e) {
    say(e.code === "EXT-012" ? "コピーはまだ使えない(EXT-012)" : `コピーできない(${e.code}: ${e.message})`);
  }
});

// 置き場から、履歴と角度を戻す
(async () => {
  try {
    const saved = await shiori.call("storage.get", { key: "history" });
    if (Array.isArray(saved)) history = saved.slice(0, HISTORY_MAX);
    const savedAngle = await shiori.call("storage.get", { key: "angle" });
    if (savedAngle === "rad" || savedAngle === "deg") setAngle(savedAngle);
  } catch (e) {
    say(`置き場を読めない(${e.code})`);
  }
  renderHistory();
})();

// 自己確認の起動(段階3)。決めた式を全部試し、1件ずつログに書いて、数を返す
let selfTestRunning = false;
window.addEventListener("message", async (event) => {
  if (event.source !== window.parent || !event.data || event.data.shiori !== 1 || !event.data.selftestRun) return;
  if (selfTestRunning) return;
  selfTestRunning = true;
  window.parent.postMessage({ shiori: 1, selftest: { started: true } }, "*");
  let passed = 0;
  for (const c of CALC_CASES) {
    const [ok, detail] = runCalcCase(ShioriCalc, c);
    if (ok) passed += 1;
    await shiori
      .call("log.write", {
        level: ok ? "info" : "warn",
        event: "試した",
        fields: { 項目: c.label, 結果: ok ? "期待どおり" : "違う", 中身: String(detail).slice(0, 160) },
      })
      .catch(() => undefined);
  }
  await shiori
    .call("log.write", {
      level: passed === CALC_CASES.length ? "info" : "warn",
      event: "全部試した",
      fields: { 期待どおり: String(passed), 全部: String(CALC_CASES.length) },
    })
    .catch(() => undefined);
  window.parent.postMessage({ shiori: 1, selftest: { ok: passed, ng: CALC_CASES.length - passed } }, "*");
  selfTestRunning = false;
});
