import { useEffect, useRef } from "react";
import { api } from "../../api/tauri";
import "./ExtensionFrame.css";

// 画面の部品の拡張機能を開く囲い(詩織Ver6.0の段階1)。
// - 拡張機能は専用のURLの仕組み(shiori-ext)で配られ、CSPでネットとほかの場所からの読み込みを止めている
// - iframeはsandbox="allow-scripts"だけにする(allow-same-originを付けないので、本体の画面にも
//   Tauriの窓口にも触れない)
// - 拡張機能からの頼みごと(postMessage)は、送り主のiframeから拡張機能の名前を決めて詩織に渡す。
//   拡張機能が名乗った名前は信じない

// WebView2(Windows)は独自の仕組みのURLを http://<名前>.localhost/ に読み替える。WKWebView(Mac)はそのまま。
export const EXT_ORIGIN = navigator.userAgent.includes("Windows")
  ? "http://shiori-ext.localhost"
  : "shiori-ext://localhost";

export function extensionUrl(name: string, entry: string): string {
  const path = entry
    .split("/")
    .map((part) => encodeURIComponent(part))
    .join("/");
  return `${EXT_ORIGIN}/${encodeURIComponent(name)}/${path}`;
}

// 開いているiframeのwindowと、拡張機能の名前の対応。頼みごとの送り主を見分けるのに使う。
const frames = new Map<Window, string>();
let listening = false;

// 自己確認の起動(段階2)。本体が {shiori: 1, selftestRun: true} を拡張機能のiframeへ送ると、
// 自分の確かめを持つ拡張機能は {shiori: 1, selftest: {started: true}} を返してから確かめを走らせ、
// 終わったら {shiori: 1, selftest: {ok, ng}} を返す。送り主のiframeから決めた名前を付けて、
// 画面の中のイベント(SELF_TEST_EVENT)にして流す。
export const SELF_TEST_EVENT = "shiori-ext-selftest";
export interface SelfTestDone {
  name: string;
  started: boolean;
  ok: number;
  ng: number;
}

interface Request {
  shiori: 1;
  id: number;
  method: string;
  params: unknown;
}

function isRequest(data: unknown): data is Request {
  const d = data as Request | null;
  return !!d && d.shiori === 1 && typeof d.id === "number" && typeof d.method === "string";
}

function listen() {
  if (listening) return;
  listening = true;
  window.addEventListener("message", (event) => {
    const source = event.source as Window | null;
    const name = source ? frames.get(source) : undefined;
    if (!source || !name) return;
    const done = (event.data as { selftest?: { started?: unknown; ok?: unknown; ng?: unknown } } | null)?.selftest;
    if ((event.data as { shiori?: number })?.shiori === 1 && done) {
      const detail: SelfTestDone = {
        name,
        started: done.started === true,
        ok: Number(done.ok) || 0,
        ng: Number(done.ng) || 0,
      };
      window.dispatchEvent(new CustomEvent(SELF_TEST_EVENT, { detail }));
      return;
    }
    if (!isRequest(event.data)) return;
    const { id, method, params } = event.data;
    api
      .extCall(name, method, params)
      .then((result) => source.postMessage({ shiori: 1, id, result }, "*"))
      .catch((err: unknown) => {
        const e = err as { code?: string; message?: string };
        const error = { code: e?.code ?? "EXT-000", message: e?.message ?? String(err) };
        source.postMessage({ shiori: 1, id, error }, "*");
      });
  });
}

interface Props {
  name: string;
  entry: string;
  title: string;
}

function ExtensionFrame({ name, entry, title }: Props) {
  const ref = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    listen();
    const win = ref.current?.contentWindow;
    if (!win) return;
    frames.set(win, name);
    return () => {
      frames.delete(win);
    };
  }, [name]);

  return (
    <iframe
      ref={ref}
      className="extension-frame"
      title={title}
      src={extensionUrl(name, entry)}
      sandbox="allow-scripts"
    />
  );
}

export default ExtensionFrame;
