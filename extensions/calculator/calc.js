// 計算機の式の読み取り(詩織Ver6.0の段階3)。囲いのCSPでevalが使えないので、字句分けと優先順位つきの読み取りを自分で行う。
// 画面(<script src="calc.js">)ではwindow.ShioriCalcに、Nodeの確かめでは読み込んだ環境のShioriCalcに置く。画面にもファイルにも触らない純粋な関数だけ。
(function (root) {
  "use strict";

  // 式が読めない(calculator-001)・計算できない(calculator-002)
  function CalcError(code, message) {
    const err = new Error(message);
    err.code = code;
    return err;
  }
  const unreadable = (m) => CalcError("calculator-001", m);
  const incalculable = (m) => CalcError("calculator-002", m);

  const FUNCS = ["asin", "acos", "atan", "sin", "cos", "tan", "log", "ln", "sqrt"];
  const CONSTS = { "π": Math.PI, pi: Math.PI, e: Math.E };
  // 見た目の違う記号を、読み取りで使う記号にそろえる
  const SAME = { "×": "*", "·": "*", "÷": "/", "−": "-", "－": "-", "＋": "+", "（": "(", "）": ")" };

  // 字句分け。{type: "num"|"op"|"func"|"const"|"ans", value}の並びにする
  function tokenize(text) {
    const tokens = [];
    let i = 0;
    while (i < text.length) {
      let ch = SAME[text[i]] || text[i];
      if (/\s/.test(ch)) { i += 1; continue; }
      if (/[0-9.]/.test(ch)) {
        let j = i;
        while (j < text.length && /[0-9.]/.test(text[j])) j += 1;
        const raw = text.slice(i, j);
        if (!/^(\d+\.?\d*|\.\d+)$/.test(raw)) throw unreadable(`数として読めない: ${raw}`);
        tokens.push({ type: "num", value: Number(raw) });
        i = j;
        continue;
      }
      if ("+-*/^%!()√".includes(ch)) { tokens.push({ type: "op", value: ch }); i += 1; continue; }
      if (ch === "π") { tokens.push({ type: "const", value: "π" }); i += 1; continue; }
      if (/[a-z]/i.test(ch)) {
        const rest = text.slice(i).toLowerCase();
        const fn = FUNCS.find((f) => rest.startsWith(f));
        if (fn) { tokens.push({ type: "func", value: fn }); i += fn.length; continue; }
        if (rest.startsWith("ans")) { tokens.push({ type: "ans" }); i += 3; continue; }
        if (rest.startsWith("pi")) { tokens.push({ type: "const", value: "pi" }); i += 2; continue; }
        if (rest[0] === "e") { tokens.push({ type: "const", value: "e" }); i += 1; continue; }
      }
      throw unreadable(`読めない文字: ${ch}`);
    }
    return tokens;
  }

  // 度のときに、90の倍数の三角関数をきっちりの値にする(sin180°が1e-16にならないように)
  const snap = (x) => (Math.abs(x) < 1e-15 ? 0 : x);

  function applyFunc(name, x, angle) {
    const toRad = angle === "deg" ? Math.PI / 180 : 1;
    const fromRad = angle === "deg" ? 180 / Math.PI : 1;
    switch (name) {
      case "sin": case "cos": case "tan": {
        if (angle === "deg" && Number.isFinite(x) && x % 90 === 0) {
          const q = (((x / 90) % 4) + 4) % 4; // 0・90・180・270度のどれか
          const table = { sin: [0, 1, 0, -1], cos: [1, 0, -1, 0] };
          if (name !== "tan") return table[name][q];
          if (q % 2 === 1) throw incalculable(`tan ${x}° は定義されない`);
          return 0;
        }
        const r = x * toRad;
        if (name === "tan") {
          if (Math.abs(Math.cos(r)) < 1e-15) throw incalculable("tan が定義されない角度");
          return snap(Math.tan(r));
        }
        return snap(name === "sin" ? Math.sin(r) : Math.cos(r));
      }
      case "asin": case "acos":
        if (x < -1 || x > 1) throw incalculable(`${name} は -1〜1 だけ`);
        return (name === "asin" ? Math.asin(x) : Math.acos(x)) * fromRad;
      case "atan": return Math.atan(x) * fromRad;
      case "log": case "ln":
        if (x <= 0) throw incalculable(`${name} は 0 より大きい数だけ`);
        return name === "log" ? Math.log10(x) : Math.log(x);
      case "sqrt":
        if (x < 0) throw incalculable("負の数の √ は計算できない");
        return Math.sqrt(x);
    }
    throw unreadable(`知らない関数: ${name}`);
  }

  function factorial(x) {
    if (!Number.isInteger(x) || x < 0) throw incalculable("! は 0 以上の整数だけ");
    if (x > 170) throw incalculable("! の答えが大きすぎる");
    let r = 1;
    for (let k = 2; k <= x; k += 1) r *= k;
    return r;
  }

  // 優先順位(低い順): + - / * / ・暗黙のかけ算 / 単項の - + / ^(右から) / 後ろの ! % / 数・定数・関数・括弧
  function evaluate(text, options) {
    const angle = (options && options.angle) === "rad" ? "rad" : "deg";
    const ans = options ? options.ans : undefined;
    const tokens = tokenize(String(text));
    if (tokens.length === 0) throw unreadable("式が空");
    let pos = 0;
    const peek = () => tokens[pos];
    const isOp = (v) => peek() && peek().type === "op" && peek().value === v;
    // 次の字句が値の始まりなら、暗黙のかけ算にする(2π、2(3)、3sin30 など)
    const startsValue = () => {
      const t = peek();
      return t && (t.type !== "op" || t.value === "(" || t.value === "√");
    };

    function expr() {
      let v = term();
      while (isOp("+") || isOp("-")) {
        const op = tokens[pos++].value;
        const r = term();
        v = op === "+" ? v + r : v - r;
      }
      return v;
    }
    function term() {
      let v = unary();
      for (;;) {
        if (isOp("*")) { pos += 1; v *= unary(); }
        else if (isOp("/")) {
          pos += 1;
          const r = unary();
          if (r === 0) throw incalculable("0 で割れない");
          v /= r;
        } else if (startsValue()) v *= unary();
        else return v;
      }
    }
    function unary() {
      if (isOp("-")) { pos += 1; return -unary(); }
      if (isOp("+")) { pos += 1; return unary(); }
      return power();
    }
    function power() {
      const base = postfix();
      if (isOp("^")) {
        pos += 1;
        const exp = unary(); // 2^-1 や 2^3^2(=2^9)を許す
        const v = Math.pow(base, exp);
        if (Number.isNaN(v)) throw incalculable("その累乗は計算できない");
        return v;
      }
      return base;
    }
    function postfix() {
      let v = primary();
      for (;;) {
        if (isOp("!")) { pos += 1; v = factorial(v); }
        else if (isOp("%")) { pos += 1; v /= 100; }
        else return v;
      }
    }
    function primary() {
      const t = tokens[pos];
      if (!t) throw unreadable("式が途中で終わっている");
      pos += 1;
      if (t.type === "num") return t.value;
      if (t.type === "const") return CONSTS[t.value];
      if (t.type === "ans") {
        if (typeof ans !== "number") throw unreadable("前の答えが無い");
        return ans;
      }
      if (t.type === "func" || (t.type === "op" && t.value === "√")) {
        // 括弧があればその中、無ければすぐ後ろの値(√2、sin30)を引数にする
        const arg = postfix();
        return applyFunc(t.type === "func" ? t.value : "sqrt", arg, angle);
      }
      if (t.type === "op" && t.value === "(") {
        const v = expr();
        if (!isOp(")")) throw unreadable("括弧が閉じていない");
        pos += 1;
        return v;
      }
      throw unreadable(`ここに ${t.value} は置けない`);
    }

    const value = expr();
    if (pos < tokens.length) throw unreadable(`余分なものがある: ${tokens[pos].value ?? "ans"}`);
    if (!Number.isFinite(value)) throw incalculable("答えが大きすぎるか、定まらない");
    return value;
  }

  // 答えを見せる形にする(有効数字12桁で丸め、末尾の0を落とす)
  function format(value) {
    if (Number.isInteger(value) && Math.abs(value) < 1e15) return String(value);
    return String(Number(value.toPrecision(12)));
  }

  root.ShioriCalc = Object.freeze({ evaluate, format, tokenize });
})(typeof window !== "undefined" ? window : globalThis);
