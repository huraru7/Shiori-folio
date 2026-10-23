// 試しの大きな画面(hello-screen)。中身は無く、列の画面に出て開けることだけを確かめる。
// 自己確認の起動では、頼まれたらログに1行書いて、期待どおり1件と返す。
const state = document.getElementById("state");
let count = 0;
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) state.textContent = `開いた回数: ${++count}`;
});

window.addEventListener("message", async (event) => {
  if (event.source !== window.parent || !event.data || event.data.shiori !== 1 || !event.data.selftestRun) return;
  window.parent.postMessage({ shiori: 1, selftest: { started: true } }, "*");
  let ok = 0;
  try {
    await shiori.call("log.write", { event: "自己確認で開かれた" });
    ok = 1;
  } catch {
    ok = 0;
  }
  window.parent.postMessage({ shiori: 1, selftest: { ok, ng: 1 - ok } }, "*");
});
