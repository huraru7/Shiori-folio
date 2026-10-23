import { api } from "../api/tauri";
import type { SelfTestMode } from "../types";
import { useMainViewStore } from "../store/useMainViewStore";
import { useSidebarStore } from "../store/useSidebarStore";
import { useExtensionsStore } from "../store/useExtensionsStore";
import { useMotionStore } from "../store/useMotionStore";
import { allItems, arrange, visibleItems, type SidebarItem } from "./sidebarItems";
import { SELF_TEST_EVENT, type SelfTestDone } from "../components/extensions/ExtensionFrame";

// 自己確認の起動(詩織Ver6.0の段階2)。`shiori-folio.exe --self-test` で起動すると、起動画面が
// 消えてから列の項目を全部開いて確かめ、1項目1行で [SELFTEST] のログに書き、最後に本体を閉じる。
// 見た目(崩れていないか、動きが気持ちよいか)は確かめられないので、ふらるに見てもらう。

let modePromise: Promise<SelfTestMode> | null = null;

export function getSelfTestMode(): Promise<SelfTestMode> {
  modePromise ??= api.selfTestMode().catch(() => ({ enabled: false, breakTarget: null }));
  return modePromise;
}

// 画面が開くまで待つ時間(検索の索引などを読みにいく画面があるため、少し長めにする)
const SCREEN_WAIT_MS = 4000;
// 拡張機能が自分の確かめを始めたと知らせるまで待つ時間と、終わるまで待つ時間
const EXT_START_WAIT_MS = 3000;
const EXT_DONE_WAIT_MS = 60000;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const nextFrames = () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));

async function waitFor(check: () => boolean, timeoutMs: number): Promise<boolean> {
  const end = performance.now() + timeoutMs;
  while (performance.now() < end) {
    if (check()) return true;
    await sleep(50);
  }
  return check();
}

async function waitForAsync(check: () => Promise<boolean>, timeoutMs: number): Promise<boolean> {
  const end = performance.now() + timeoutMs;
  while (performance.now() < end) {
    if (await check()) return true;
    await sleep(100);
  }
  return check();
}

// 拡張機能に自分の確かめを頼み、終わるまで待つ。自分の確かめを持たない拡張機能(始めたと返さない)はnull。
// iframeがまだ読み込み中のこともあるので、始めたと返すまで頼み直す。
async function askExtension(iframe: HTMLIFrameElement | null, name: string): Promise<SelfTestDone | null> {
  if (!iframe) return null;
  let started = false;
  let done: SelfTestDone | null = null;
  const onEvent = (e: Event) => {
    const d = (e as CustomEvent<SelfTestDone>).detail;
    if (d.name !== name) return;
    if (d.started) started = true;
    else done = d;
  };
  window.addEventListener(SELF_TEST_EVENT, onEvent);
  try {
    const end = performance.now() + EXT_START_WAIT_MS;
    while (!started && !done && performance.now() < end) {
      iframe.contentWindow?.postMessage({ shiori: 1, selftestRun: true }, "*");
      await sleep(300);
    }
    if (!started && !done) return null;
    await waitFor(() => done !== null, EXT_DONE_WAIT_MS);
    return done ?? { name, started: true, ok: 0, ng: 1 };
  } finally {
    window.removeEventListener(SELF_TEST_EVENT, onEvent);
  }
}

export async function runSelfTest(): Promise<void> {
  // 画面の部品のエラー(console.errorと、捕まえられなかった例外)を集める
  const errors: string[] = [];
  const originalError = console.error;
  console.error = (...args: unknown[]) => {
    errors.push(args.map((a) => (a instanceof Error ? a.message : String(a))).join(" ").slice(0, 200));
    originalError(...args);
  };
  const onError = (e: ErrorEvent) => errors.push(String(e.message).slice(0, 200));
  const onRejection = (e: PromiseRejectionEvent) => errors.push(String(e.reason).slice(0, 200));
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);

  let ok = 0;
  let ng = 0;
  const report = async (item: string, pass: boolean, fields: Record<string, string | number | boolean> = {}) => {
    if (pass) ok += 1;
    else ng += 1;
    const strings = Object.fromEntries(Object.entries(fields).map(([k, v]) => [k, String(v)]));
    await api.selfTestLog(item, pass ? "OK" : "NG", strings).catch(() => undefined);
  };

  try {
    const loaded = await waitFor(
      () => useSidebarStore.getState().loaded && useExtensionsStore.getState().list !== null,
      10000,
    );
    await report("読み込み", loaded, { 列の並び: useSidebarStore.getState().loaded, 拡張機能: useExtensionsStore.getState().list !== null });

    // 拡張機能の読み込みの状態
    for (const ext of useExtensionsStore.getState().list ?? []) {
      const stopped = ext.runtime?.state === "stopped";
      await report(`拡張機能:${ext.folder}`, ext.status === "ready" && !stopped, {
        状態: ext.status,
        種類: ext.kind ?? "-",
        置き場所: ext.kind === "ui" ? ext.slot : "-",
        動き: ext.runtime?.state ?? "-",
        ...(ext.errorCode && { エラー: ext.errorCode }),
      });
    }

    const { layout, iconOverrides } = useSidebarStore.getState();
    const all = arrange(allItems(useExtensionsStore.getState().list ?? [], iconOverrides), layout);
    const visible = visibleItems(all, layout);
    await checkSidebarShape(visible, report);

    for (const item of all.filter((i) => i.group !== "tool")) {
      await checkScreen(item, visible, errors, report);
    }
    for (const item of all.filter((i) => i.group === "tool")) {
      await checkTool(item, visible, errors, report);
    }
    await checkSettingsPage(report);
    await checkEffects(report);
  } catch (e) {
    await report("自己確認の途中", false, { 内容: String(e).slice(0, 200) });
  } finally {
    useMainViewStore.getState().closeTool();
    useMainViewStore.getState().setActive("home");
    console.error = originalError;
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
    await api.selfTestFinish(ok, ng).catch(() => undefined);
  }
}

type Report = (item: string, pass: boolean, fields?: Record<string, string | number | boolean>) => Promise<void>;

// 列の形: 上から画面、線、小道具、いちばん下。上のタブは無い。アイコンが読めている。
async function checkSidebarShape(visible: SidebarItem[], report: Report) {
  await nextFrames();
  const buttons = Array.from(document.querySelectorAll<HTMLElement>(".sidebar [data-sidebar-id]"));
  const ids = buttons.map((b) => b.dataset.sidebarId);
  const expected = visible.map((i) => i.id);
  const sameOrder = ids.join(",") === expected.join(",");
  const hasTools = visible.some((i) => i.group === "tool");
  const lineOk = !hasTools || document.querySelector(".sidebar .sidebar__line") !== null;
  // 段階1までの上のタブ(画面の中のタブ〈Claudeモニターなど〉は数えない)
  const noTabs = document.querySelector(".main-area__bar, .main-area__tabs") === null;
  await report("列の形", sameOrder && lineOk && noTabs, {
    並び: ids.join(","),
    線: lineOk,
    上のタブが無い: noTabs,
  });

  const imgs = Array.from(document.querySelectorAll<HTMLImageElement>(".sidebar img.sidebar__icon"));
  await waitFor(() => imgs.every((i) => i.complete), 3000);
  const broken = imgs.filter((i) => i.naturalWidth === 0).map((i) => i.closest<HTMLElement>("[data-sidebar-id]")?.dataset.sidebarId);
  const sizes = Array.from(new Set(imgs.map((i) => `${i.naturalWidth}x${i.naturalHeight}`))).join(",");
  const pixelated = imgs.every((i) => getComputedStyle(i).imageRendering === "pixelated");
  // 出している大きさが、画面の実際の画素で24ドットの整数倍か
  const dpr = window.devicePixelRatio || 1;
  const shownPx = imgs[0]?.getBoundingClientRect().width ?? 0;
  const devicePx = shownPx * dpr;
  const integerScale = imgs.length === 0 || Math.abs(devicePx / 24 - Math.round(devicePx / 24)) < 0.01;
  await report("列のアイコン", broken.length === 0 && pixelated && integerScale, {
    数: imgs.length,
    読めない: broken.join(",") || "なし",
    大きさ: sizes,
    にじませない: pixelated,
    出している大きさ: `${shownPx.toFixed(1)}px(画素${Math.round(devicePx)} 倍率${dpr} 窓の高さ${window.innerHeight})`,
    整数倍: integerScale,
  });

  // 名前の表示: 指を乗せた項目の右に、項目と同じ高さで出る(最初・最後の項目で確かめる)
  const tipChecks = [buttons[0], buttons[buttons.length - 1]].filter(Boolean);
  const tipResults: string[] = [];
  let tipOk = true;
  for (const b of tipChecks) {
    b.dispatchEvent(new MouseEvent("mouseover", { bubbles: true, relatedTarget: document.body }));
    await nextFrames();
    const tip = document.querySelector<HTMLElement>(".sidebar__tip");
    const br = b.getBoundingClientRect();
    const tr = tip?.getBoundingClientRect();
    const ok =
      !!tr &&
      Math.abs(tr.top + tr.height / 2 - (br.top + br.height / 2)) < 2 &&
      tr.left >= br.right &&
      tr.left - br.right < 20;
    tipOk &&= ok;
    tipResults.push(`${b.dataset.sidebarId}:${ok ? "合う" : "ずれる"}`);
    b.dispatchEvent(new MouseEvent("mouseout", { bubbles: true, relatedTarget: document.body }));
    await nextFrames();
  }
  const tipGone = document.querySelector(".sidebar__tip") === null;
  await report("列の名前の表示", tipOk && tipGone, { 位置: tipResults.join(","), 離すと消える: tipGone });
}

// 画面を1つ開いて確かめる。列に出ていれば列のボタンを押し、隠していれば直接選ぶ。
async function checkScreen(item: SidebarItem, visible: SidebarItem[], errors: string[], report: Report) {
  const before = errors.length;
  const start = performance.now();
  const button = document.querySelector<HTMLElement>(`.sidebar [data-sidebar-id="${CSS.escape(item.id)}"]`);
  const shown = visible.includes(item);
  if (shown && button) button.click();
  else useMainViewStore.getState().setActive(item.id);

  const pane = () => document.querySelector<HTMLElement>(`[data-view-id="${CSS.escape(item.id)}"]`);
  const opened = await waitFor(() => {
    const p = pane();
    if (!p || p.hidden) return false;
    if (p.querySelector(".error-boundary")) return true;
    return (p.textContent ?? "").trim().length > 0 || p.querySelector("iframe") !== null;
  }, SCREEN_WAIT_MS);
  const ms = Math.round(performance.now() - start);
  const p = pane();
  const boundary = p?.querySelector(".error-boundary__message")?.textContent ?? null;
  const othersHidden = Array.from(document.querySelectorAll<HTMLElement>("[data-view-id]")).every(
    (el) => el === p || el.hidden,
  );

  const fields: Record<string, string | number | boolean> = {
    名前: item.label,
    ms,
    列に出ている: shown && !!button,
    ほかは隠れている: othersHidden,
  };
  let conversationPass = true;
  if (item.id === "conversation") {
    fields.形 = p?.querySelector<HTMLElement>("[data-conversation-mode]")?.dataset.conversationMode ?? "-";
    // ナレッジは会話の画面の右側にある
    const knowledge = p?.querySelector(".conversation-screen__knowledge .knowledge-panel") ?? null;
    fields.ナレッジ = knowledge !== null;
    conversationPass = knowledge !== null;
  }

  let extensionPass = true;
  if (item.ext) {
    const done = await askExtension(p?.querySelector("iframe") ?? null, item.ext.name);
    if (done) {
      fields.拡張機能の自己確認 = `期待どおり${done.ok} 違う${done.ng}`;
      extensionPass = done.ng === 0 && done.ok > 0;
    }
  }

  await sleep(150);
  const newErrors = errors.slice(before);
  fields.エラー = newErrors.length;
  if (boundary) fields.画面のエラー = boundary.slice(0, 120);
  if (newErrors.length > 0) fields.エラーの内容 = newErrors[0];
  await report(
    `画面:${item.id}`,
    opened && !boundary && othersHidden && extensionPass && conversationPass && newErrors.length === 0,
    fields,
  );
}

// 小道具を開いて閉じる。いまの画面は切り替わらず、閉じても中身(iframe)が残ることを確かめる。
async function checkTool(item: SidebarItem, visible: SidebarItem[], errors: string[], report: Report) {
  if (!item.ext) return;
  const name = item.ext.name;
  const before = errors.length;
  const store = useMainViewStore.getState();
  store.setActive("home");
  await nextFrames();
  const button = document.querySelector<HTMLElement>(`.sidebar [data-sidebar-id="${CSS.escape(item.id)}"]`);
  const shown = visible.includes(item) && !!button;
  if (shown) button!.click();
  else store.toggleTool(name);

  const frame = () => document.querySelector<HTMLElement>(".tool-frame");
  const slot = () => document.querySelector<HTMLElement>(`.tool-frame__slot[data-tool-name="${CSS.escape(name)}"]`);
  const opened = await waitFor(() => !!frame() && !frame()!.hidden && !!slot()?.querySelector("iframe"), SCREEN_WAIT_MS);
  const iframe = slot()?.querySelector("iframe") ?? null;
  const stayedOnHome = useMainViewStore.getState().active === "home";
  const done = opened ? await askExtension(iframe, name) : null;

  // もう一度押すと閉じる
  if (shown) document.querySelector<HTMLElement>(`.sidebar [data-sidebar-id="${CSS.escape(item.id)}"]`)?.click();
  else useMainViewStore.getState().toggleTool(name);
  await nextFrames();
  const closed = frame()?.hidden === true;
  const kept = !!iframe && iframe.isConnected && slot()?.querySelector("iframe") === iframe;

  const newErrors = errors.slice(before);
  const fields: Record<string, string | number | boolean> = {
    名前: item.label,
    開いた: opened,
    画面はそのまま: stayedOnHome,
    閉じた: closed,
    閉じても中身が残る: kept,
    エラー: newErrors.length,
  };
  let extensionPass = true;
  if (done) {
    fields.拡張機能の自己確認 = `期待どおり${done.ok} 違う${done.ng}`;
    extensionPass = done.ng === 0 && done.ok > 0;
  }
  if (newErrors.length > 0) fields.エラーの内容 = newErrors[0];
  await report(`小道具:${name}`, opened && stayedOnHome && closed && kept && extensionPass && newErrors.length === 0, fields);
}

// 設定の画面の「列と演出」: ボタンで並べ替え・非表示をすると、列が変わり、記録(sidebar.json)に書かれる。
// 確かめたあとは、もとの記録に戻す。
async function checkSettingsPage(report: Report) {
  const original = await api.getSidebarLayout();
  useMainViewStore.getState().setActive("settings");
  await nextFrames();
  document.querySelector<HTMLElement>('[data-nav-key="sidebar"]')?.click();
  const shown = await waitFor(() => document.querySelector("[data-sidebar-settings-id]") !== null, SCREEN_WAIT_MS);
  const columnIds = () =>
    Array.from(document.querySelectorAll<HTMLElement>(".sidebar [data-sidebar-id]")).map((b) => b.dataset.sidebarId);
  const firstScreen = document.querySelector<HTMLElement>('[data-sidebar-settings-group="screen"] [data-sidebar-settings-id]');
  const id = firstScreen?.dataset.sidebarSettingsId ?? "";
  // 1つ目の画面を下へ
  firstScreen?.querySelector<HTMLButtonElement>('button[aria-label$="を下へ"]')?.click();
  await waitFor(() => columnIds()[1] === id, 2000);
  const movedInColumn = columnIds()[1] === id;
  const movedSaved = await waitForAsync(async () => (await api.getSidebarLayout()).order[1] === id, 2000);
  // その画面を隠す
  const row = () => document.querySelector<HTMLElement>(`[data-sidebar-settings-id="${CSS.escape(id)}"]`);
  row()?.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click();
  await waitFor(() => !columnIds().includes(id), 2000);
  const hiddenInColumn = !columnIds().includes(id);
  const hiddenSaved = await waitForAsync(async () => (await api.getSidebarLayout()).hidden.includes(id), 2000);
  // 設定と拡張機能は隠せない
  const lockedOk = ["settings", "extensions"].every(
    (b) =>
      document.querySelector<HTMLInputElement>(`[data-sidebar-settings-id="${b}"] input[type="checkbox"]`)?.disabled === true,
  );
  // もとに戻す
  await api.setSidebarLayout(original);
  await useSidebarStore.getState().load();
  await nextFrames();
  const restored = (await api.getSidebarLayout()).order.join(",") === original.order.join(",");
  await report("設定:列と演出", shown && movedInColumn && movedSaved && hiddenInColumn && hiddenSaved && lockedOk && restored, {
    動かした項目: id,
    列で下へ動いた: movedInColumn,
    記録に並び: movedSaved,
    列から隠れた: hiddenInColumn,
    記録に非表示: hiddenSaved,
    設定と拡張機能は隠せない: lockedOk,
    もとに戻した: restored,
  });

  // デバッグは、ほかのページと同じく設定の画面の中で開く(全画面に切り替わらない)
  document.querySelector<HTMLElement>('[data-nav-key="debug"]')?.click();
  const settingsPane = document.querySelector<HTMLElement>('[data-view-id="settings"]');
  const debugShown = await waitFor(
    () => (settingsPane?.querySelector(".control-panel__main h1")?.textContent ?? "") === "デバッグ",
    SCREEN_WAIT_MS,
  );
  const stillInApp = document.querySelector(".sidebar") !== null && settingsPane?.hidden === false;
  await report("設定:デバッグ", debugShown && stillInApp, { 設定の中で開いた: debugShown, 列が出たまま: stillInApp });
  document.querySelector<HTMLElement>('[data-nav-key="system"]')?.click();
  useMainViewStore.getState().setActive("home");
}

// 演出: 動きを抑えていなければ、星が出ていて、押したときに水紋が出る。抑えていれば、どちらも出ない。
async function checkEffects(report: Report) {
  const m = useMotionStore.getState();
  const reduce = m.configured ?? m.osReduce;
  const stars = document.querySelectorAll(".effects__star").length;
  const button = document.querySelector<HTMLElement>(".sidebar [data-sidebar-id]");
  let ripples = 0;
  if (button) {
    const rect = button.getBoundingClientRect();
    const init = { bubbles: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2 };
    button.dispatchEvent(new PointerEvent("pointerdown", init));
    await nextFrames();
    ripples = document.querySelectorAll(".effects__ripple").length;
  }
  const pass = reduce ? stars === 0 && ripples === 0 : stars > 0 && ripples > 0;
  await report("演出", pass, {
    動きを抑える: reduce,
    設定: m.configured === null ? "OSに従う" : String(m.configured),
    星: stars,
    水紋: ripples,
  });
}
