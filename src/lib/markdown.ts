import type { MouseEvent } from "react";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { openUrl } from "@tauri-apps/plugin-opener";

// 記事(library)の本文の表示(詩織Ver4.0)。libraryには外部AIやshiori-saveも書き込むため、
// 記事の中身は信頼できない入力として扱う。markedの出力にはHTMLがそのまま通るので、
// DOMPurifyでスクリプト・イベント属性・javascript:のURLなどを取り除いてから埋め込む。
// 画面の見た目を乗っ取れるstyle(タグと属性)と、ほかのページを埋め込む要素も外す。
export function renderMarkdown(markdown: string): string {
  const html = marked.parse(markdown, { async: false });
  return DOMPurify.sanitize(html, {
    FORBID_TAGS: ["style", "iframe", "frame", "object", "embed", "form", "input", "button"],
    FORBID_ATTR: ["style"],
  });
}

// 記事内のリンクを押したとき、詩織の画面がそのページへ移動しないよう横取りする。
// http(s)のリンクだけを既定のブラウザで開き、それ以外(相対パスなど)は何もしない。
export function handleArticleLinkClick(event: MouseEvent<HTMLElement>): void {
  const anchor = (event.target as HTMLElement).closest("a");
  if (!anchor) return;
  event.preventDefault();
  const href = anchor.getAttribute("href") ?? "";
  if (/^https?:\/\//i.test(href)) {
    openUrl(href).catch((e) => console.error("リンクを開けませんでした:", e));
  }
}
