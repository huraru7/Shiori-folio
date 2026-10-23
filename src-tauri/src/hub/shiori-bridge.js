// 拡張機能が詩織に頼みごとをするための部品(詩織Ver6.0の段階1)。
// 拡張機能のページで <script src="/_shiori/shiori.js"></script> と読み込み、
//   const result = await shiori.call("log.write", { level: "info", event: "押された" });
// のように使う。頼みごとは本体の画面へpostMessageで送り、本体の画面が詩織(ハブ)に渡す。
// 断られたときは、code(例: "EXT-011")とmessageを持つErrorでrejectする。
(() => {
  let nextId = 1;
  const pending = new Map();

  window.addEventListener("message", (event) => {
    // 本体の画面(親)からの返事だけを受け取る
    if (event.source !== window.parent) return;
    const msg = event.data;
    if (!msg || msg.shiori !== 1 || !pending.has(msg.id)) return;
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.error) {
      const err = new Error(msg.error.message);
      err.code = msg.error.code;
      reject(err);
    } else {
      resolve(msg.result);
    }
  });

  window.shiori = Object.freeze({
    call(method, params) {
      const id = nextId++;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        // 囲いの中は送り主の場所が「無い」(opaque)ので、宛先は"*"にする。親は送り主のiframeで見分ける
        window.parent.postMessage({ shiori: 1, id, method, params: params ?? {} }, "*");
      });
    },
  });
})();
