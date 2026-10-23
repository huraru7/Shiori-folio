// タイマー・ストップウォッチの画面(詩織Ver6.0の段階3)。長さと時刻の読み取りは timeparse.js。
// タイマーは詩織に時刻を預け(schedule.add)、時間が来たら詩織が知らせる。この画面を閉じても、
// ほかの画面にいても知らせが届く。一覧は詩織の予約(schedule.list)から作り直す。
// ストップウォッチは、始めた時刻を置き場に書き、開き直しても続きから数える。

const whenEl = document.getElementById("when");
const labelEl = document.getElementById("label");
const messageEl = document.getElementById("message");
const timersEl = document.getElementById("timers");
const emptyEl = document.getElementById("empty");
const elapsedEl = document.getElementById("elapsed");
const lapsEl = document.getElementById("laps");
const watchStartEl = document.getElementById("watch-start");
const watchLapEl = document.getElementById("watch-lap");
const watchResetEl = document.getElementById("watch-reset");

function say(text) {
  messageEl.textContent = text;
}

// ---- 切り替え ----

for (const [tab, pane] of [
  ["tab-timer", "pane-timer"],
  ["tab-watch", "pane-watch"],
]) {
  document.getElementById(tab).addEventListener("click", () => {
    for (const t of document.querySelectorAll(".tab")) {
      const on = t.id === tab;
      t.classList.toggle("active", on);
      t.setAttribute("aria-selected", String(on));
    }
    document.getElementById("pane-timer").hidden = pane !== "pane-timer";
    document.getElementById("pane-watch").hidden = pane !== "pane-watch";
  });
}

// ---- タイマー ----

let reservations = []; // [{id, at, label}] 詩織に預けた予約

// 行は予約が増えたり減ったりしたときだけ作り直し、それ以外は残りの時間の文字だけを書き換える
// (1秒に10回作り直すと、押している間にボタンが入れ替わり、押しても届かないことがあった)
let renderedKey = "";
const leftEls = new Map(); // 予約のid → 残りの時間を出す要素

function renderTimers() {
  const now = Date.now();
  reservations = reservations.filter((r) => r.at > now);
  const key = reservations.map((r) => `${r.id}:${r.at}:${r.label}`).join("|");
  if (key !== renderedKey) {
    renderedKey = key;
    leftEls.clear();
    timersEl.textContent = "";
    for (const r of reservations) {
      const li = document.createElement("li");
      const left = document.createElement("span");
      left.className = "left";
      leftEls.set(r.id, left);
      const what = document.createElement("span");
      what.className = "what";
      const d = new Date(r.at);
      const clock = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
      what.textContent = r.label ? `${r.label}(${clock})` : `${clock}に知らせる`;
      const cancel = document.createElement("button");
      cancel.type = "button";
      cancel.className = "cancel";
      cancel.textContent = "取り消す";
      cancel.addEventListener("click", () => cancelTimer(r.id));
      li.append(left, what, cancel);
      timersEl.append(li);
    }
    emptyEl.hidden = reservations.length > 0;
  }
  for (const r of reservations) {
    const el = leftEls.get(r.id);
    const text = ShioriTime.formatLeft(r.at - now);
    if (el && el.textContent !== text) el.textContent = text;
  }
}

async function refreshTimers() {
  try {
    reservations = await shiori.call("schedule.list", {});
  } catch (e) {
    say(`予約を読めない(${e.code})`);
  }
  renderTimers();
}

async function startTimer(text) {
  let at;
  try {
    at = ShioriTime.parse(text, Date.now()).at;
  } catch (e) {
    say(`${e.code}: ${e.message}`);
    return;
  }
  try {
    await shiori.call("schedule.add", { at, label: labelEl.value.trim() });
    whenEl.value = "";
    labelEl.value = "";
    say("");
  } catch (e) {
    // 予約の数の上限は、本体のEXT-025を、この拡張機能のtimer-002として出す
    say(e.code === "EXT-025" ? "timer-002: 予約の数が多すぎる(20件まで)" : `予約できない(${e.code}: ${e.message})`);
  }
  await refreshTimers();
}

async function cancelTimer(id) {
  try {
    await shiori.call("schedule.cancel", { id });
  } catch (e) {
    // もう終わっていた(EXT-026)ときは、一覧を作り直すだけ
    if (e.code !== "EXT-026") say(`取り消せない(${e.code})`);
  }
  await refreshTimers();
}

// 囲い(iframe)の中ではフォームの送信が禁じられていて、submitの合図が来ない。
// そのため、ボタンとEnterで直接始める(日本語の変換を確定するEnterでは始めない)
const startEl = document.getElementById("start");
startEl.addEventListener("click", () => startTimer(whenEl.value));
for (const el of [whenEl, labelEl]) {
  el.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.isComposing) {
      e.preventDefault();
      startTimer(whenEl.value);
    }
  });
}
for (const b of document.querySelectorAll("[data-quick]")) {
  b.addEventListener("click", () => startTimer(b.dataset.quick));
}

// ---- ストップウォッチ ----

// running のときは startAt からの経過を accumulated に足して数える
let watch = { running: false, startAt: 0, accumulated: 0, laps: [] };

function elapsed() {
  return watch.accumulated + (watch.running ? Date.now() - watch.startAt : 0);
}

function saveWatch() {
  shiori.call("storage.set", { key: "watch", value: watch }).catch((e) => say(`ストップウォッチを保存できない(${e.code})`));
}

function renderWatch() {
  elapsedEl.textContent = ShioriTime.formatElapsed(elapsed());
  watchStartEl.textContent = watch.running ? "止める" : elapsed() > 0 ? "続ける" : "始める";
  watchLapEl.disabled = !watch.running;
  watchResetEl.disabled = watch.running || elapsed() === 0;
  lapsEl.textContent = "";
  watch.laps.forEach((ms, i) => {
    const li = document.createElement("li");
    const prev = i + 1 < watch.laps.length ? watch.laps[i + 1] : 0;
    const n = document.createElement("span");
    n.textContent = `ラップ${watch.laps.length - i}`;
    const t = document.createElement("span");
    t.textContent = `${ShioriTime.formatElapsed(ms - prev)} / ${ShioriTime.formatElapsed(ms)}`;
    li.append(n, t);
    lapsEl.append(li);
  });
}

watchStartEl.addEventListener("click", () => {
  if (watch.running) {
    watch = { ...watch, running: false, accumulated: elapsed() };
  } else {
    watch = { ...watch, running: true, startAt: Date.now() };
  }
  saveWatch();
  renderWatch();
});
watchLapEl.addEventListener("click", () => {
  watch = { ...watch, laps: [elapsed(), ...watch.laps].slice(0, 99) };
  saveWatch();
  renderWatch();
});
watchResetEl.addEventListener("click", () => {
  watch = { running: false, startAt: 0, accumulated: 0, laps: [] };
  saveWatch();
  renderWatch();
});

// ---- 動かし始め ----

(async () => {
  try {
    const saved = await shiori.call("storage.get", { key: "watch" });
    if (saved && typeof saved === "object" && Array.isArray(saved.laps)) watch = saved;
  } catch (e) {
    say(`置き場を読めない(${e.code})`);
  }
  renderWatch();
  await refreshTimers();
})();

// 残りの時間は画面の中で数え、預けた予約は時々読み直す(ほかの形から足した予約も出すため)
setInterval(() => {
  renderTimers();
  if (watch.running) elapsedEl.textContent = ShioriTime.formatElapsed(elapsed());
}, 100);
setInterval(refreshTimers, 5000);

// ---- 自己確認の起動(段階3) ----
// 決めた読み取りを全部試し、詩織に本当に預けて、一覧に出ることと取り消せることも確かめる

async function liveChecks() {
  const results = [];
  // 欄に打って「始める」を押すと、詩織に預けられる(画面の操作から通す)
  whenEl.value = "59分";
  labelEl.value = "自己確認の操作";
  startEl.click();
  let byButton = null;
  for (let i = 0; i < 30 && !byButton; i++) {
    await new Promise((r) => setTimeout(r, 100));
    byButton = (await shiori.call("schedule.list", {})).find((r) => r.label === "自己確認の操作") ?? null;
  }
  results.push(["欄に打って始めるを押す", byButton !== null, byButton ? `id=${byButton.id}` : messageEl.textContent || "予約されない"]);
  // 一覧の「取り消す」は、残りの時間が変わっても同じボタンのまま(入れ替わると押しても届かない)
  await refreshTimers();
  const before = timersEl.querySelector(".cancel");
  await new Promise((r) => setTimeout(r, 1200));
  const after = timersEl.querySelector(".cancel");
  results.push(["取り消すのボタンが入れ替わらない", !!before && before === after, before ? "" : "ボタンが無い"]);
  if (byButton) await shiori.call("schedule.cancel", { id: byButton.id });

  const label = "自己確認";
  const added = await shiori.call("schedule.add", { at: Date.now() + 60_000, label });
  let list = await shiori.call("schedule.list", {});
  results.push(["予約が一覧に出る", list.some((r) => r.id === added.id && r.label === label), `id=${added.id}`]);
  await shiori.call("schedule.cancel", { id: added.id });
  list = await shiori.call("schedule.list", {});
  results.push(["取り消すと一覧から消える", !list.some((r) => r.id === added.id), `残り${list.length}件`]);
  let code = "";
  try {
    await shiori.call("schedule.cancel", { id: added.id });
  } catch (e) {
    code = e.code;
  }
  results.push(["取り消した予約はもう無い", code === "EXT-026", code || "エラーにならない"]);
  await refreshTimers();
  return results;
}

let selfTestRunning = false;
window.addEventListener("message", async (event) => {
  if (event.source !== window.parent || !event.data || event.data.shiori !== 1 || !event.data.selftestRun) return;
  if (selfTestRunning) return;
  selfTestRunning = true;
  window.parent.postMessage({ shiori: 1, selftest: { started: true } }, "*");
  const results = TIMER_CASES.map((c) => [c.label, ...runTimerCase(ShioriTime, c)]);
  try {
    results.push(...(await liveChecks()));
  } catch (e) {
    results.push(["詩織への予約", false, `${e.code}: ${e.message}`]);
  }
  let passed = 0;
  for (const [label, ok, detail] of results) {
    if (ok) passed += 1;
    await shiori
      .call("log.write", {
        level: ok ? "info" : "warn",
        event: "試した",
        fields: { 項目: label, 結果: ok ? "期待どおり" : "違う", 中身: String(detail).slice(0, 160) },
      })
      .catch(() => undefined);
  }
  await shiori
    .call("log.write", {
      level: passed === results.length ? "info" : "warn",
      event: "全部試した",
      fields: { 期待どおり: String(passed), 全部: String(results.length) },
    })
    .catch(() => undefined);
  window.parent.postMessage({ shiori: 1, selftest: { ok: passed, ng: results.length - passed } }, "*");
  selfTestRunning = false;
});
