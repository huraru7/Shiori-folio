// 変換(converter)の計算の部分。画面にもログにも触らない、純粋な関数だけを置く。
// 画面(index.html)では普通のscriptとして読み、globalThis.ShioriConvert から使う。
// Nodeの確かめ(convert.test.js)では、vmで同じファイルを読んで同じ関数を使う。
// 囲いのCSPでeval・new Functionは使えないので、使わない。
(() => {
  // エラー。codeは説明書(extension.json)のerrorsと同じ
  class ConvertError extends Error {
    constructor(code, message) {
      super(message);
      this.code = code;
    }
  }

  // ---- 単位 ----
  // factor は「その単位の1つが、基準の単位でいくつか」。温度だけは比で表せないので toBase/fromBase を持つ。
  // 尺貫法の値は計量法施行法(昭和26年法律第208号)の第4条・第5条の定義による(尺=10/33 m、坪=400/121 m²、
  // 升=2401/1331000 m³、貫=3.75 kg、寸=尺の1/10、匁=貫の1/1000、合=升の1/10)。
  // ヤード・ポンド法は国際ヤード・ポンド(1959年)、acreは国際エーカー、US galは231立方インチ。
  const SHAKU = 10 / 33; // m
  const SHO = 2401 / 1331; // L
  const INCH = 0.0254; // m
  const FOOT = 0.3048; // m
  const MILE = 1609.344; // m
  const US_GAL = 3.785411784; // L

  const CATEGORIES = [
    {
      id: "length",
      label: "長さ",
      base: "m",
      units: [
        { id: "mm", label: "mm", factor: 0.001 },
        { id: "cm", label: "cm", factor: 0.01 },
        { id: "m", label: "m", factor: 1 },
        { id: "km", label: "km", factor: 1000 },
        { id: "inch", label: "inch", factor: INCH },
        { id: "ft", label: "ft", factor: FOOT },
        { id: "yd", label: "yd", factor: 0.9144 },
        { id: "mile", label: "mile", factor: MILE },
        { id: "shaku", label: "尺", factor: SHAKU },
        { id: "sun", label: "寸", factor: SHAKU / 10 },
      ],
    },
    {
      id: "mass",
      label: "重さ",
      base: "g",
      units: [
        { id: "mg", label: "mg", factor: 0.001 },
        { id: "g", label: "g", factor: 1 },
        { id: "kg", label: "kg", factor: 1000 },
        { id: "t", label: "t", factor: 1e6 },
        { id: "oz", label: "oz", factor: 28.349523125 },
        { id: "lb", label: "lb", factor: 453.59237 },
        { id: "kan", label: "貫", factor: 3750 },
        { id: "momme", label: "匁", factor: 3.75 },
      ],
    },
    {
      id: "temperature",
      label: "温度",
      base: "K",
      units: [
        { id: "C", label: "℃", toBase: (v) => v + 273.15, fromBase: (k) => k - 273.15 },
        { id: "F", label: "℉", toBase: (v) => ((v - 32) * 5) / 9 + 273.15, fromBase: (k) => ((k - 273.15) * 9) / 5 + 32 },
        { id: "K", label: "K", toBase: (v) => v, fromBase: (k) => k },
      ],
    },
    {
      id: "data",
      label: "データ量",
      base: "B",
      units: [
        { id: "bit", label: "bit", factor: 1 / 8 },
        { id: "B", label: "B", factor: 1 },
        { id: "KB", label: "KB", factor: 1e3 },
        { id: "MB", label: "MB", factor: 1e6 },
        { id: "GB", label: "GB", factor: 1e9 },
        { id: "TB", label: "TB", factor: 1e12 },
        { id: "KiB", label: "KiB", factor: 1024 },
        { id: "MiB", label: "MiB", factor: 1024 ** 2 },
        { id: "GiB", label: "GiB", factor: 1024 ** 3 },
        { id: "TiB", label: "TiB", factor: 1024 ** 4 },
      ],
    },
    {
      id: "time",
      label: "時間",
      base: "s",
      units: [
        { id: "ms", label: "ミリ秒", factor: 0.001 },
        { id: "s", label: "秒", factor: 1 },
        { id: "min", label: "分", factor: 60 },
        { id: "h", label: "時間", factor: 3600 },
        { id: "d", label: "日", factor: 86400 },
        { id: "w", label: "週", factor: 604800 },
      ],
    },
    {
      id: "speed",
      label: "速度",
      base: "m/s",
      units: [
        { id: "mps", label: "m/s", factor: 1 },
        { id: "kmh", label: "km/h", factor: 1000 / 3600 },
        { id: "mph", label: "mph", factor: MILE / 3600 },
        { id: "knot", label: "ノット", factor: 1852 / 3600 },
      ],
    },
    {
      id: "area",
      label: "面積",
      base: "m²",
      units: [
        { id: "mm2", label: "mm²", factor: 1e-6 },
        { id: "cm2", label: "cm²", factor: 1e-4 },
        { id: "m2", label: "m²", factor: 1 },
        { id: "km2", label: "km²", factor: 1e6 },
        { id: "a", label: "a", factor: 100 },
        { id: "ha", label: "ha", factor: 1e4 },
        // 畳は法の定義が無く、地域で大きさが違う。企画書のとおり1.62m²(不動産の表示の目安)とする
        { id: "jo", label: "畳", factor: 1.62 },
        { id: "tsubo", label: "坪", factor: 400 / 121 },
        { id: "ft2", label: "ft²", factor: FOOT * FOOT },
        { id: "acre", label: "acre", factor: 4046.8564224 },
      ],
    },
    {
      id: "volume",
      label: "体積",
      base: "L",
      units: [
        { id: "mL", label: "mL", factor: 0.001 },
        { id: "L", label: "L", factor: 1 },
        { id: "m3", label: "m³", factor: 1000 },
        { id: "cm3", label: "cm³", factor: 0.001 },
        { id: "go", label: "合", factor: SHO / 10 },
        { id: "sho", label: "升", factor: SHO },
        { id: "usgal", label: "US gal", factor: US_GAL },
        // US customary cup = 1/16 US gal(約236.6mL)。食品表示のlegal cup(240mL)とは別
        { id: "cup", label: "cup(US)", factor: US_GAL / 16 },
      ],
    },
  ];

  function findCategory(categoryId) {
    const category = CATEGORIES.find((c) => c.id === categoryId);
    if (!category) throw new Error(`知らない種類: ${categoryId}`);
    return category;
  }

  function findUnit(category, unitId) {
    const unit = category.units.find((u) => u.id === unitId);
    if (!unit) throw new Error(`知らない単位: ${category.id}/${unitId}`);
    return unit;
  }

  function toBase(unit, value) {
    return unit.toBase ? unit.toBase(value) : value * unit.factor;
  }

  function fromBase(unit, value) {
    return unit.fromBase ? unit.fromBase(value) : value / unit.factor;
  }

  // 全角の数字・記号を半角に直し、桁区切り(, _ 空白)を取る
  function normalize(text) {
    return String(text)
      .replace(/[０-９＋－．，]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) - 0xfee0))
      .replace(/[−ー]/g, "-")
      .replace(/[,_\s]/g, "");
  }

  // 入った文字を数にする。空ならnull(何も出さない)、数として読めなければconverter-001
  function parseNumber(text) {
    const s = normalize(text);
    if (s === "") return null;
    if (!/^[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i.test(s)) {
      throw new ConvertError("converter-001", `数として読めない: ${String(text).slice(0, 40)}`);
    }
    const value = Number(s);
    if (!Number.isFinite(value)) {
      throw new ConvertError("converter-001", "大きすぎて数として扱えない");
    }
    return value;
  }

  // 1つの値を、ほかの1つの単位へ
  function convert(categoryId, fromId, toId, value) {
    const category = findCategory(categoryId);
    return fromBase(findUnit(category, toId), toBase(findUnit(category, fromId), value));
  }

  // 1つの値を、同じ種類の全部の単位へ。[{id, label, value}]
  function convertAll(categoryId, fromId, value) {
    const category = findCategory(categoryId);
    const base = toBase(findUnit(category, fromId), value);
    return category.units.map((u) => ({ id: u.id, label: u.label, value: fromBase(u, base) }));
  }

  // 画面に出す形とコピーする形。有効数字12桁で丸めて、浮動小数の端数(0.30000000000000004など)を消す
  function formatNumber(value) {
    if (!Number.isFinite(value)) return "—";
    if (value === 0) return "0";
    const rounded = Number(value.toPrecision(12));
    const abs = Math.abs(rounded);
    if (abs >= 1e15 || abs < 1e-9) {
      // とても大きい・小さい数は指数で出す(1.5e+20 の形)
      return rounded.toExponential(11).replace(/\.?0+e/, "e");
    }
    // 1e-7より小さいとStringが指数にするので、そこは桁を並べる
    if (abs < 1e-6) return rounded.toFixed(20).replace(/\.?0+$/, "");
    return String(rounded);
  }

  // ---- 基数 ----
  const RADIXES = [
    { radix: 2, label: "2進", digits: /^[01]+$/, prefix: /^0b/i, jsPrefix: "0b" },
    { radix: 8, label: "8進", digits: /^[0-7]+$/, prefix: /^0o/i, jsPrefix: "0o" },
    { radix: 10, label: "10進", digits: /^[0-9]+$/, prefix: null, jsPrefix: "" },
    { radix: 16, label: "16進", digits: /^[0-9a-f]+$/i, prefix: /^0x/i, jsPrefix: "0x" },
  ];

  function findRadix(radix) {
    const r = RADIXES.find((x) => x.radix === radix);
    if (!r) throw new Error(`知らない基数: ${radix}`);
    return r;
  }

  // その基数の文字を、0以上の整数(BigInt)にする。空ならnull。使えない文字があればconverter-002
  function parseInteger(text, radix) {
    const r = findRadix(radix);
    let s = normalize(text);
    if (r.prefix) s = s.replace(r.prefix, "");
    if (s === "") return null;
    if (!r.digits.test(s)) {
      throw new ConvertError("converter-002", `${r.label}で使えない文字がある: ${String(text).slice(0, 40)}`);
    }
    // BigIntは文字から直接作れる(0b/0o/0xの頭で基数を伝える)。桁は落ちない
    return BigInt(r.jsPrefix + s);
  }

  // 0以上の整数(BigInt)を、その基数の文字にする。16進は大文字
  function formatInteger(value, radix) {
    findRadix(radix);
    const s = value.toString(radix);
    return radix === 16 ? s.toUpperCase() : s;
  }

  // 1つの基数で打たれた文字から、4つの基数の文字を全部作る。{2:"…", 8:"…", 10:"…", 16:"…"} か、空ならnull
  function convertRadixAll(text, radix) {
    const value = parseInteger(text, radix);
    if (value === null) return null;
    const out = {};
    for (const r of RADIXES) out[r.radix] = formatInteger(value, r.radix);
    return out;
  }

  globalThis.ShioriConvert = Object.freeze({
    ConvertError,
    CATEGORIES,
    RADIXES,
    findCategory,
    parseNumber,
    convert,
    convertAll,
    formatNumber,
    parseInteger,
    formatInteger,
    convertRadixAll,
  });
})();
