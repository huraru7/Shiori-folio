// 試しの画面(hello-ui)。段階1の受け入れ基準のうち、画面の部品の囲いと窓口を確かめる。
// 「期待どおり」かどうかを、1行ずつ表に出す。
const check = document.getElementById("script-check");
check.textContent = "スクリプトが動いた(app.jsとshiori.jsが読めた)";
check.className = window.shiori ? "ok" : "ng";

// 試すことの一覧。runは、期待どおりならtrueと、表に出す説明を返す。
const TESTS = [
  {
    label: "ログに書く(log.write)",
    expect: "書ける",
    run: async () => {
      await shiori.call("log.write", { event: "試しの画面から書いた", fields: { 回: "1" } });
      return [true, "詩織のログに [hello-ui] で出る"];
    },
  },
  {
    label: "説明書に無い clipboard.write",
    expect: "EXT-011で断られる",
    run: () => expectRefused("clipboard.write", { text: "x" }, "EXT-011"),
  },
  {
    label: "説明書にあるが、まだ作っていない storage.get",
    expect: "EXT-012で断られる(ステップ6で作る)",
    run: () => expectRefused("storage.get", { key: "a" }, "EXT-012"),
  },
  {
    label: "詩織が知らない頼みごと",
    expect: "EXT-010で断られる",
    run: () => expectRefused("fs.read", { path: "C:/" }, "EXT-010"),
  },
  {
    label: "ネットへの接続(fetch https://example.com)",
    expect: "失敗する",
    run: () => expectFetchFails("https://example.com/"),
  },
  {
    label: "Tauriの窓口への直接の接続(fetch http://ipc.localhost)",
    expect: "失敗する",
    run: () => expectFetchFails("http://ipc.localhost/plugin%3Aapp%7Cversion"),
  },
  {
    label: "本体の画面(親)の中身に触る",
    expect: "触れない",
    run: async () => {
      try {
        const title = window.parent.document.title;
        return [false, `触れてしまった: ${title}`];
      } catch (e) {
        return [true, String(e)];
      }
    },
  },
  {
    label: "Tauriの内部の部品(__TAURI_INTERNALS__)がある(記録だけ)",
    expect: "どちらでもよい(窓口を呼べなければ穴ではない)",
    run: async () => {
      const t = typeof window.__TAURI_INTERNALS__;
      const keys = t === "object" ? Object.keys(window.__TAURI_INTERNALS__).join(",") : "";
      return [true, `typeof = ${t} ${keys}`];
    },
  },
  {
    label: "Tauriの部品で本体の窓口を呼ぶ(invoke list_extensions)",
    expect: "失敗する",
    run: () => expectInvokeFails("list_extensions", {}),
  },
  {
    label: "Tauriの部品で、別の名前を名乗ってext_callを呼ぶ",
    expect: "失敗する",
    run: () => expectInvokeFails("ext_call", { name: "other", method: "log.write", params: { event: "なりすまし" } }),
  },
];

async function expectRefused(method, params, code) {
  try {
    await shiori.call(method, params);
    return [false, "断られなかった"];
  } catch (e) {
    return [e.code === code, `${e.code}: ${e.message}`];
  }
}

// Tauriの内部の部品が囲いの中にあっても、本体の窓口を呼べなければよい。5秒で返事が無ければ失敗とみなす
async function expectInvokeFails(cmd, args) {
  const internals = window.__TAURI_INTERNALS__;
  if (!internals || typeof internals.invoke !== "function") {
    return [true, "invokeが無い"];
  }
  try {
    const result = await Promise.race([
      internals.invoke(cmd, args),
      new Promise((_, reject) => setTimeout(() => reject(new Error("5秒で返事が無い")), 5000)),
    ]);
    return [false, `呼べてしまった: ${JSON.stringify(result).slice(0, 200)}`];
  } catch (e) {
    return [true, String(e && e.message ? e.message : e)];
  }
}

async function expectFetchFails(url) {
  try {
    const res = await fetch(url);
    return [false, `つながってしまった: ${res.status}`];
  } catch (e) {
    return [true, String(e)];
  }
}

function addRow(test, ok, detail) {
  const tr = document.createElement("tr");
  const cells = [test.label, test.expect, ok ? "期待どおり" : "違う"];
  for (const text of cells) {
    const td = document.createElement("td");
    td.textContent = text;
    tr.appendChild(td);
  }
  tr.lastChild.className = ok ? "ok" : "ng";
  const detailRow = document.createElement("tr");
  const td = document.createElement("td");
  td.colSpan = 3;
  td.className = "detail";
  td.textContent = detail;
  detailRow.appendChild(td);
  results.append(tr, detailRow);
}

const results = document.getElementById("results");
document.getElementById("run-all").addEventListener("click", async () => {
  results.textContent = "";
  let passed = 0;
  for (const test of TESTS) {
    let ok = false;
    let detail = "";
    try {
      [ok, detail] = await test.run();
    } catch (e) {
      detail = `試す途中で失敗: ${e}`;
    }
    if (ok) passed += 1;
    addRow(test, ok, detail);
  }
  // 結果の数もログに残す(ふらるが画面を読まなくても、ログで確かめられるように)
  await shiori
    .call("log.write", {
      level: passed === TESTS.length ? "info" : "warn",
      event: "全部試した",
      fields: { 期待どおり: String(passed), 全部: String(TESTS.length) },
    })
    .catch(() => undefined);
});
