/**
 * Advanced Calculator: tính biểu thức ngay trên thanh search.
 * Hỗ trợ + - * / ^ ( ) %, hàm sin/cos/tan/sqrt/log/ln/abs..., hằng số pi, e.
 * "100 * 20%" = 20 (phần trăm hậu tố). Gõ "=" ở đầu để ép tính (VD: "=pi*5^2").
 * Lượng giác dùng radian.
 */

const FUNCS: Record<string, (x: number) => number> = {
  sin: Math.sin,
  cos: Math.cos,
  tan: Math.tan,
  asin: Math.asin,
  acos: Math.acos,
  atan: Math.atan,
  sqrt: Math.sqrt,
  abs: Math.abs,
  log: Math.log10,
  ln: Math.log,
  log2: Math.log2,
  exp: Math.exp,
  round: Math.round,
  floor: Math.floor,
  ceil: Math.ceil,
};

const CONSTS: Record<string, number> = {
  pi: Math.PI,
  e: Math.E,
  tau: Math.PI * 2,
};

class Parser {
  private pos = 0;
  constructor(private readonly src: string) {}

  parse(): number {
    const v = this.expr();
    this.skipWs();
    if (this.pos < this.src.length) throw new Error("ký tự thừa");
    return v;
  }

  private expr(): number {
    let v = this.term();
    for (;;) {
      this.skipWs();
      const c = this.src[this.pos];
      if (c === "+" || c === "-") {
        this.pos++;
        const rhs = this.term();
        v = c === "+" ? v + rhs : v - rhs;
      } else return v;
    }
  }

  private term(): number {
    let v = this.factor();
    for (;;) {
      this.skipWs();
      const c = this.src[this.pos];
      if (c === "*" || c === "/") {
        this.pos++;
        const rhs = this.factor();
        v = c === "*" ? v * rhs : v / rhs;
      } else return v;
    }
  }

  private factor(): number {
    const base = this.unary();
    this.skipWs();
    if (this.src[this.pos] === "^") {
      this.pos++;
      return Math.pow(base, this.factor());
    }
    return base;
  }

  private unary(): number {
    this.skipWs();
    if (this.src[this.pos] === "-") {
      this.pos++;
      return -this.unary();
    }
    return this.postfix();
  }

  private postfix(): number {
    let v = this.primary();
    this.skipWs();
    while (this.src[this.pos] === "%") {
      this.pos++;
      v /= 100;
      this.skipWs();
    }
    return v;
  }

  private primary(): number {
    this.skipWs();
    if (this.src[this.pos] === "(") {
      this.pos++;
      const v = this.expr();
      this.skipWs();
      if (this.src[this.pos] !== ")") throw new Error("thiếu )");
      this.pos++;
      return v;
    }
    const num = /^\d+(\.\d+)?/.exec(this.src.slice(this.pos));
    if (num) {
      this.pos += num[0].length;
      return parseFloat(num[0]);
    }
    const ident = /^[a-zA-Z]+[0-9]?/.exec(this.src.slice(this.pos));
    if (ident) {
      this.pos += ident[0].length;
      const name = ident[0].toLowerCase();
      if (name in CONSTS) return CONSTS[name];
      const fn = FUNCS[name];
      if (!fn) throw new Error(`không biết "${name}"`);
      this.skipWs();
      if (this.src[this.pos] !== "(") throw new Error("hàm cần (");
      this.pos++;
      const arg = this.expr();
      this.skipWs();
      if (this.src[this.pos] !== ")") throw new Error("thiếu )");
      this.pos++;
      return fn(arg);
    }
    throw new Error("không phải số");
  }

  private skipWs() {
    while (this.src[this.pos] === " ") this.pos++;
  }
}

function formatNumber(v: number): string {
  if (Number.isInteger(v) && Math.abs(v) < 1e15) return v.toLocaleString("en-US");
  const rounded = parseFloat(v.toPrecision(10));
  return rounded.toString();
}

export function tryCalculate(raw: string): string | null {
  let input = raw.trim();
  const forced = input.startsWith("=");
  if (forced) input = input.slice(1).trim();
  if (!input) return null;
  if (!/^[\d\s+\-*/%^().,a-zA-Z]+$/.test(input)) return null;
  // Không ép "=" thì phải có số VÀ toán tử/ngoặc — tránh nuốt query tên app
  if (!forced && !/\d/.test(input)) return null;
  if (!forced && !/[+*/%^(]|\d\s*-/.test(input)) return null;
  input = input.replace(/,/g, "");
  try {
    const v = new Parser(input).parse();
    if (!isFinite(v)) return null;
    return formatNumber(v);
  } catch {
    return null;
  }
}
