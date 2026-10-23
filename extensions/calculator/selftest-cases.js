// 自己確認とNodeの確かめで使う、決めた式と答え(詩織Ver6.0の段階3)。
// want は format した答え、error は期待するエラーコード。
(function (root) {
  "use strict";
  root.CALC_CASES = Object.freeze([
    { label: "優先順位(かけ算が先)", expr: "1+2×3", want: "7" },
    { label: "括弧", expr: "(1+2)×3", want: "9" },
    { label: "割り算と引き算", expr: "10−4÷2", want: "8" },
    { label: "小数", expr: "0.1+0.2", want: "0.3" },
    { label: "累乗は右から", expr: "2^3^2", want: "512" },
    { label: "単項のマイナスより累乗が先", expr: "-2^2", want: "-4" },
    { label: "負の指数", expr: "2^-1", want: "0.5" },
    { label: "%", expr: "50%×200", want: "100" },
    { label: "√", expr: "√16+√(9)", want: "7" },
    { label: "階乗", expr: "5!", want: "120" },
    { label: "π と暗黙のかけ算", expr: "2π", want: "6.28318530718" },
    { label: "e", expr: "ln(e)", want: "1" },
    { label: "log(10を底)", expr: "log(1000)", want: "3" },
    { label: "sin(度)", expr: "sin(30)", want: "0.5" },
    { label: "cos(度)", expr: "cos(60)", want: "0.5" },
    { label: "tan(度)", expr: "tan(45)", want: "1" },
    { label: "sin180°はきっちり0", expr: "sin(180)", want: "0" },
    { label: "asin(度)", expr: "asin(1)", want: "90" },
    { label: "sin(ラジアン)", expr: "sin(π÷2)", angle: "rad", want: "1" },
    { label: "acos(ラジアン)", expr: "acos(-1)", angle: "rad", want: "3.14159265359" },
    { label: "0で割る", expr: "1÷0", error: "calculator-002" },
    { label: "tan90°", expr: "tan(90)", error: "calculator-002" },
    { label: "負の数のlog", expr: "log(-1)", error: "calculator-002" },
    { label: "小数の階乗", expr: "2.5!", error: "calculator-002" },
    { label: "括弧が閉じていない", expr: "(1+2", error: "calculator-001" },
    { label: "読めない文字", expr: "1+$", error: "calculator-001" },
    { label: "演算子で終わる", expr: "3×", error: "calculator-001" },
  ]);

  // 1件試して [期待どおりか, 説明] を返す
  root.runCalcCase = function (calc, c) {
    try {
      const got = calc.format(calc.evaluate(c.expr, { angle: c.angle || "deg" }));
      return c.error ? [false, `${got}(${c.error}になるはず)`] : [got === c.want, `${c.expr} = ${got}`];
    } catch (e) {
      return c.error ? [e.code === c.error, `${e.code}: ${e.message}`] : [false, `${e.code}: ${e.message}`];
    }
  };
})(typeof window !== "undefined" ? window : globalThis);
