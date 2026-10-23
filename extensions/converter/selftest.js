// 変換(converter)の確かめの中身。決めた値の換算と、基数の往復。
// 画面の自己確認(app.js)とNodeの確かめ(convert.test.js)の両方が、この一覧をそのまま走らせる。
// runは ShioriConvert を受け取り、期待どおりならtrueと、表やログに出す説明を返す。
(() => {
  // 浮動小数の比べ方(相対で1e-12まで)
  function near(actual, expected) {
    if (expected === 0) return Math.abs(actual) < 1e-12;
    return Math.abs(actual - expected) / Math.abs(expected) < 1e-12;
  }

  // 単位の換算の確かめ。[種類, 元, 値, 先, 期待]
  const UNIT_CASES = [
    ["length", "inch", 1, "cm", 2.54],
    ["length", "mile", 1, "km", 1.609344],
    ["length", "ft", 1, "inch", 12],
    ["length", "shaku", 33, "m", 10],
    ["length", "shaku", 1, "sun", 10],
    ["mass", "lb", 1, "g", 453.59237],
    ["mass", "lb", 1, "oz", 16],
    ["mass", "kan", 1, "kg", 3.75],
    ["mass", "kan", 1, "momme", 1000],
    ["mass", "t", 1, "mg", 1e9],
    ["temperature", "C", 0, "F", 32],
    ["temperature", "C", 0, "K", 273.15],
    ["temperature", "C", 100, "F", 212],
    ["temperature", "C", -40, "F", -40],
    ["temperature", "F", 98.6, "C", 37],
    ["temperature", "K", 0, "C", -273.15],
    ["temperature", "F", 0, "K", 255.37222222222223],
    ["data", "GB", 1, "B", 1e9],
    ["data", "GiB", 1, "B", 1073741824],
    ["data", "B", 1, "bit", 8],
    ["data", "MiB", 1, "MB", 1.048576],
    ["data", "TiB", 1, "GiB", 1024],
    ["time", "d", 1, "s", 86400],
    ["time", "w", 1, "h", 168],
    ["time", "min", 1, "ms", 60000],
    ["speed", "kmh", 36, "mps", 10],
    ["speed", "knot", 1, "kmh", 1.852],
    ["speed", "mph", 1, "kmh", 1.609344],
    ["area", "tsubo", 121, "m2", 400],
    ["area", "jo", 1, "m2", 1.62],
    ["area", "ha", 1, "a", 100],
    ["area", "acre", 1, "m2", 4046.8564224],
    ["area", "km2", 1, "ha", 100],
    ["volume", "sho", 1, "go", 10],
    ["volume", "sho", 1331, "L", 2401],
    ["volume", "usgal", 1, "cup", 16],
    ["volume", "usgal", 1, "L", 3.785411784],
    ["volume", "m3", 1, "L", 1000],
  ];

  // 基数の確かめ。[打った文字, 基数, 期待する4つ]
  const RADIX_CASES = [
    ["255", 10, { 2: "11111111", 8: "377", 10: "255", 16: "FF" }],
    ["ff", 16, { 2: "11111111", 8: "377", 10: "255", 16: "FF" }],
    ["0x1A", 16, { 2: "11010", 8: "32", 10: "26", 16: "1A" }],
    ["0", 2, { 2: "0", 8: "0", 10: "0", 16: "0" }],
    // 2^64(Numberでは桁が落ちる大きさ)
    ["18446744073709551616", 10, { 2: "1" + "0".repeat(64), 8: "2" + "0".repeat(21), 10: "18446744073709551616", 16: "1" + "0".repeat(16) }],
  ];

  // エラーになるはずのもの。[説明, 試す関数, 期待するコード]
  const ERROR_CASES = [
    ["数として読めない「abc」", (C) => C.parseNumber("abc"), "converter-001"],
    ["数として読めない「1.2.3」", (C) => C.parseNumber("1.2.3"), "converter-001"],
    ["2進に「102」", (C) => C.parseInteger("102", 2), "converter-002"],
    ["8進に「8」", (C) => C.parseInteger("8", 8), "converter-002"],
    ["10進に負の数「-5」", (C) => C.parseInteger("-5", 10), "converter-002"],
    ["10進に小数「1.5」", (C) => C.parseInteger("1.5", 10), "converter-002"],
    ["16進に「G」", (C) => C.parseInteger("G", 16), "converter-002"],
  ];

  const TESTS = [];

  for (const [category, from, value, to, expected] of UNIT_CASES) {
    TESTS.push({
      label: `換算 ${category}: ${value} ${from} → ${to}`,
      expect: String(expected),
      run: (C) => {
        const actual = C.convert(category, from, to, value);
        return [near(actual, expected), `${actual}`];
      },
    });
  }

  TESTS.push({
    label: "読み取り: 全角・桁区切り・指数",
    expect: "「１，２３４．５」=1234.5、「1e3」=1000、空=null",
    run: (C) => {
      const a = C.parseNumber("１，２３４．５");
      const b = C.parseNumber("1e3");
      const c = C.parseNumber("  ");
      return [a === 1234.5 && b === 1000 && c === null, `${a} / ${b} / ${c}`];
    },
  });

  TESTS.push({
    label: "全部の単位へ並べる(長さ 1m)",
    expect: "10個並び、mは1",
    run: (C) => {
      const rows = C.convertAll("length", "m", 1);
      const m = rows.find((r) => r.id === "m");
      return [rows.length === 10 && m.value === 1, rows.map((r) => `${r.label}=${C.formatNumber(r.value)}`).join(" ")];
    },
  });

  TESTS.push({
    label: "出す形: 浮動小数の端数を消す",
    expect: "0.1+0.2→0.3、1尺→0.303030303030 m",
    run: (C) => {
      const a = C.formatNumber(0.1 + 0.2);
      const b = C.formatNumber(C.convert("length", "shaku", "m", 1));
      return [a === "0.3" && b === "0.30303030303", `${a} / ${b}`];
    },
  });

  for (const [text, radix, expected] of RADIX_CASES) {
    TESTS.push({
      label: `基数 ${radix}進「${text.length > 12 ? text.slice(0, 12) + "…" : text}」から4つへ`,
      expect: `16進=${expected[16]}`,
      run: (C) => {
        const out = C.convertRadixAll(text, radix);
        const ok = [2, 8, 10, 16].every((r) => out[r] === expected[r]);
        return [ok, JSON.stringify(out)];
      },
    });
  }

  TESTS.push({
    label: "基数の往復(大きな数: 10進→16進→2進→8進→10進)",
    expect: "元の数に戻る(桁が落ちない)",
    run: (C) => {
      const start = "123456789012345678901234567890123456789";
      const hex = C.convertRadixAll(start, 10)[16];
      const bin = C.convertRadixAll(hex, 16)[2];
      const oct = C.convertRadixAll(bin, 2)[8];
      const back = C.convertRadixAll(oct, 8)[10];
      return [back === start, `16進=${hex} → 戻り=${back}`];
    },
  });

  for (const [label, fn, code] of ERROR_CASES) {
    TESTS.push({
      label: `エラー: ${label}`,
      expect: `${code}になる`,
      run: (C) => {
        try {
          fn(C);
          return [false, "エラーにならなかった"];
        } catch (e) {
          return [e.code === code, `${e.code}: ${e.message}`];
        }
      },
    });
  }

  globalThis.ShioriConvertTests = Object.freeze(TESTS);
})();
