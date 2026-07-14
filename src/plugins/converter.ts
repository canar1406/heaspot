/**
 * Unit Converter — đổi MỌI đơn vị cùng bản chất.
 * Các đơn vị được chia theo NHÓM BẢN CHẤT (length, mass, time, …); chỉ đổi
 * được trong cùng nhóm. Đổi chéo giữa hai nhóm sẽ báo lỗi rõ ràng thay vì
 * âm thầm bỏ qua. Toàn bộ tính toán chạy cục bộ nên tức thời.
 *
 * Ví dụ: "conv 10 ft to m", "conv 5 kg sang lb", "conv 100 f to c",
 *        "conv 2 gb to mb", "conv 60 km/h to m/s", "conv 1 atm to psi"
 */
import type { ResultItemData } from "../types";

type Category =
  | "length" | "mass" | "time" | "data" | "temp" | "area" | "volume"
  | "speed" | "pressure" | "energy" | "power" | "angle" | "frequency";

const LABEL: Record<Category, string> = {
  length: "chiều dài", mass: "khối lượng", time: "thời gian", data: "dữ liệu",
  temp: "nhiệt độ", area: "diện tích", volume: "thể tích", speed: "tốc độ",
  pressure: "áp suất", energy: "năng lượng", power: "công suất", angle: "góc",
  frequency: "tần số",
};

/** Bảng hệ số quy đổi về đơn vị gốc SI của mỗi nhóm (trừ temp — xử lý riêng). */
const TABLES: Record<Exclude<Category, "temp">, Record<string, number>> = {
  // gốc: mét
  length: {
    m: 1, km: 1000, dm: 0.1, cm: 0.01, mm: 0.001, um: 1e-6, nm: 1e-9,
    mi: 1609.344, ft: 0.3048, in: 0.0254, yd: 0.9144, nmi: 1852, ly: 9.4607e15,
  },
  // gốc: kilôgam
  mass: {
    kg: 1, g: 0.001, mg: 1e-6, ug: 1e-9, t: 1000,
    lb: 0.45359237, oz: 0.028349523125, st: 6.35029318, ct: 0.0002,
  },
  // gốc: giây
  time: {
    s: 1, ms: 0.001, us: 1e-6, ns: 1e-9, min: 60, h: 3600,
    day: 86400, week: 604800, month: 2629800, year: 31557600,
  },
  // gốc: byte (nhị phân 1024)
  data: {
    b: 1, kb: 1024, mb: 1024 ** 2, gb: 1024 ** 3, tb: 1024 ** 4, pb: 1024 ** 5,
    bit: 1 / 8, kbit: 1024 / 8, mbit: 1024 ** 2 / 8, gbit: 1024 ** 3 / 8,
  },
  // gốc: mét vuông
  area: {
    m2: 1, km2: 1e6, cm2: 1e-4, mm2: 1e-6, ha: 1e4, are: 100,
    acre: 4046.8564224, ft2: 0.09290304, in2: 0.00064516, mi2: 2589988.110336, yd2: 0.83612736,
  },
  // gốc: lít
  volume: {
    l: 1, ml: 0.001, m3: 1000, cm3: 0.001, dm3: 1,
    gal: 3.785411784, qt: 0.946352946, pt: 0.473176473, cup: 0.2365882365,
    floz: 0.0295735295625, tbsp: 0.01478676478, tsp: 0.004928921594,
    ft3: 28.316846592, in3: 0.016387064,
  },
  // gốc: mét/giây
  speed: {
    mps: 1, kmh: 0.277777777778, mph: 0.44704, ftps: 0.3048, knot: 0.514444444444,
  },
  // gốc: pascal
  pressure: {
    pa: 1, kpa: 1000, hpa: 100, bar: 1e5, mbar: 100, atm: 101325,
    psi: 6894.757293168, mmhg: 133.322387415, torr: 133.3223684211, inhg: 3386.389,
  },
  // gốc: joule
  energy: {
    j: 1, kj: 1000, mj: 1e6, cal: 4.184, kcal: 4184, wh: 3600, kwh: 3.6e6,
    ev: 1.602176634e-19, btu: 1055.05585262,
  },
  // gốc: watt
  power: {
    w: 1, kw: 1000, mw: 1e6, gw: 1e9, hp: 745.6998715823, ps: 735.49875,
  },
  // gốc: độ
  angle: {
    deg: 1, rad: 57.29577951308, grad: 0.9, arcmin: 1 / 60, arcsec: 1 / 3600, turn: 360,
  },
  // gốc: hertz
  frequency: {
    hz: 1, khz: 1000, mhz: 1e6, ghz: 1e9, thz: 1e12, rpm: 1 / 60,
  },
};

/** key canonical -> nhóm bản chất (dựng sẵn để tra O(1)). */
const KEY_CAT: Record<string, Category> = (() => {
  const map: Record<string, Category> = { c: "temp", f: "temp", k: "temp" };
  for (const cat of Object.keys(TABLES) as Array<Exclude<Category, "temp">>) {
    for (const key of Object.keys(TABLES[cat])) map[key] = cat;
  }
  return map;
})();

/** Hiển thị đẹp cho các key có ký hiệu đặc biệt. */
const DISP: Record<string, string> = {
  um: "µm", ug: "µg", us: "µs", mps: "m/s", kmh: "km/h", ftps: "ft/s",
  m2: "m²", km2: "km²", cm2: "cm²", mm2: "mm²", ft2: "ft²", in2: "in²",
  yd2: "yd²", mi2: "mi²", m3: "m³", cm3: "cm³", dm3: "dm³", ft3: "ft³", in3: "in³",
  c: "°C", f: "°F", k: "K", deg: "°",
};
const disp = (k: string) => DISP[k] ?? k;

/** Chuẩn hoá chuỗi đơn vị người dùng gõ về key canonical. */
const ALIAS: Record<string, string> = {
  // length
  meter: "m", meters: "m", metre: "m", metres: "m", "mét": "m",
  kilometer: "km", kilometers: "km", kilometre: "km", "kilômét": "km",
  centimeter: "cm", centimetre: "cm", millimeter: "mm", millimetre: "mm",
  micrometer: "um", "µm": "um", "μm": "um", nanometer: "nm",
  mile: "mi", miles: "mi", feet: "ft", foot: "ft", "'": "ft",
  inch: "in", inches: "in", '"': "in", yard: "yd", yards: "yd",
  "nautical mile": "nmi", lightyear: "ly", "light-year": "ly",
  // mass
  kilogram: "kg", kilograms: "kg", kilo: "kg", kilos: "kg",
  gram: "g", grams: "g", gramme: "g",
  milligram: "mg", microgram: "ug", "µg": "ug", "μg": "ug",
  ton: "t", tonne: "t", tonnes: "t", tan: "t", "tấn": "t",
  pound: "lb", pounds: "lb", lbs: "lb", ounce: "oz", ounces: "oz",
  stone: "st", carat: "ct", carats: "ct",
  // time
  second: "s", seconds: "s", sec: "s", secs: "s", giay: "s", "giây": "s",
  millisecond: "ms", microsecond: "us", "µs": "us", "μs": "us", nanosecond: "ns",
  minute: "min", minutes: "min", mins: "min", phut: "min", "phút": "min",
  hour: "h", hours: "h", hr: "h", hrs: "h", gio: "h", "giờ": "h",
  day: "day", days: "day", ngay: "day", "ngày": "day",
  week: "week", weeks: "week", tuan: "week", "tuần": "week",
  month: "month", months: "month", thang: "month", "tháng": "month",
  year: "year", years: "year", yr: "year", nam: "year", "năm": "year",
  // data
  byte: "b", bytes: "b", bits: "bit",
  kilobyte: "kb", megabyte: "mb", gigabyte: "gb", terabyte: "tb", petabyte: "pb",
  kilobit: "kbit", megabit: "mbit", gigabit: "gbit",
  // area
  "m²": "m2", "m2": "m2", sqm: "m2", "km²": "km2", "km2": "km2",
  "cm²": "cm2", "cm2": "cm2", "mm²": "mm2", "mm2": "mm2",
  "ft²": "ft2", "ft2": "ft2", sqft: "ft2", "in²": "in2", "in2": "in2",
  "yd²": "yd2", "yd2": "yd2", "mi²": "mi2", "mi2": "mi2",
  hectare: "ha", hecta: "ha", "héc-ta": "ha", acres: "acre", "a": "are",
  // volume
  liter: "l", litre: "l", liters: "l", litres: "l", lit: "l", "lít": "l",
  milliliter: "ml", millilitre: "ml", cc: "cm3",
  "m³": "m3", "m3": "m3", "cm³": "cm3", "cm3": "cm3", "dm³": "dm3", "dm3": "dm3",
  "ft³": "ft3", "ft3": "ft3", "in³": "in3", "in3": "in3",
  gallon: "gal", gallons: "gal", quart: "qt", pint: "pt",
  cups: "cup", "fluid ounce": "floz", "fl oz": "floz",
  tablespoon: "tbsp", teaspoon: "tsp",
  // speed
  "m/s": "mps", "mét/giây": "mps", "km/h": "kmh", kph: "kmh", kmph: "kmh",
  "ft/s": "ftps", fps: "ftps", knots: "knot", kn: "knot",
  // pressure
  pascal: "pa", kilopascal: "kpa", hectopascal: "hpa", millibar: "mbar",
  atmosphere: "atm", "mm hg": "mmhg", "in hg": "inhg",
  // energy
  joule: "j", joules: "j", kilojoule: "kj", megajoule: "mj",
  calorie: "cal", cals: "cal", kilocalorie: "kcal", calo: "cal", "kcalo": "kcal",
  "watt hour": "wh", "kilowatt hour": "kwh", electronvolt: "ev",
  // power
  watt: "w", watts: "w", kilowatt: "kw", megawatt: "mw", gigawatt: "gw",
  horsepower: "hp", "mã lực": "hp",
  // angle
  degree: "deg", degrees: "deg", "°": "deg", "do": "deg", "độ": "deg",
  radian: "rad", radians: "rad", gradian: "grad", revolution: "turn", turns: "turn",
  // frequency
  hertz: "hz", kilohertz: "khz", megahertz: "mhz", gigahertz: "ghz", terahertz: "thz",
  // temperature
  celsius: "c", "°c": "c", "độ c": "c", "do c": "c",
  fahrenheit: "f", "°f": "f", "độ f": "f", "do f": "f",
  kelvin: "k",
};

function normalize(u: string): string {
  const low = u.trim().toLowerCase();
  if (ALIAS[low]) return ALIAS[low];
  // rút gọn ² ³ về 2 3 rồi tra lại
  const flat = low.replace(/²/g, "2").replace(/³/g, "3");
  return ALIAS[flat] ?? flat;
}

function convertTemp(v: number, from: string, to: string): number | null {
  const toC = (x: number, u: string) =>
    u === "c" ? x : u === "f" ? ((x - 32) * 5) / 9 : u === "k" ? x - 273.15 : null;
  const fromC = (x: number, u: string) =>
    u === "c" ? x : u === "f" ? (x * 9) / 5 + 32 : u === "k" ? x + 273.15 : null;
  const c = toC(v, from);
  if (c == null) return null;
  return fromC(c, to);
}

function fmt(v: number): string {
  if (!isFinite(v)) return String(v);
  const r = parseFloat(v.toPrecision(8));
  if (r !== 0 && Math.abs(r) < 1e-4) return r.toExponential(4);
  return Math.abs(r) >= 1e15 ? r.toExponential(4) : r.toLocaleString("en-US");
}

const CONNECT = `to|sang|ra|thành|=|->|→|in`;
// Capture unit tự do rồi kiểm tra qua KEY_CAT. Nhờ vậy các alias Unicode/có
// khoảng trắng như "mét", "độ c", "nautical mile", "watt hour" hoạt động.
const CONV_RE = new RegExp(`^(-?[\\d.,]+(?:[kmb](?=\\s))?)\\s*(.+?)\\s+(?:${CONNECT})\\s+(.+)$`, "iu");

function parseNumber(raw: string): number {
  let s = raw.trim();
  // Hậu tố gõ tắt dính số: 1k=1.000, 1m=1 triệu, 1b=1 tỉ
  let mult = 1;
  const suf = /^(-?[\d.,]+)([kmb])$/i.exec(s);
  if (suf) {
    s = suf[1];
    const u = suf[2].toLowerCase();
    mult = u === "k" ? 1e3 : u === "m" ? 1e6 : 1e9;
  }
  const normalized = s.includes(",") && !s.includes(".")
    ? (/^-?\d{1,3}(,\d{3})+$/.test(s) ? s.replace(/,/g, "") : s.replace(",", "."))
    : s.replace(/,/g, "");
  return Number.parseFloat(normalized) * mult;
}

export function tryConvert(q: string): ResultItemData | null {
  const query = q.trim();

  // Đổi hệ cơ số (bin/hex/dec/ascii) là nhóm riêng, thử trước
  const data = tryDataConvert(query);
  if (data) return data;

  const m = CONV_RE.exec(query);
  if (!m) return null;
  const value = parseNumber(m[1]);
  if (!isFinite(value)) return null;
  const from = normalize(m[2]);
  const to = normalize(m[3]);

  const catFrom = KEY_CAT[from];
  const catTo = KEY_CAT[to];
  // Một trong hai không phải đơn vị đã biết -> để search khác xử lý
  if (!catFrom || !catTo) return null;

  // Chặn đổi chéo giữa hai nhóm bản chất khác nhau
  if (catFrom !== catTo) {
    return {
      id: `unit:err:${query}`,
      title: `Không đổi được: ${disp(from)} (${LABEL[catFrom]}) ≠ ${disp(to)} (${LABEL[catTo]})`,
      subtitle: "Hai đơn vị khác bản chất — chỉ đổi được trong cùng nhóm",
      kind: "unit",
      text: "",
    };
  }

  let out: number | null;
  if (catFrom === "temp") {
    out = convertTemp(value, from, to);
  } else {
    const table = TABLES[catFrom];
    out = (value * table[from]) / table[to];
  }
  if (out == null || !isFinite(out)) return null;

  const outStr = fmt(out);
  return {
    id: `unit:${query}`,
    title: `${m[1]} ${disp(from)} = ${outStr} ${disp(to)}`,
    subtitle: `Đổi ${LABEL[catFrom]} — Enter để copy`,
    kind: "unit",
    text: outStr,
  };
}

// ---------------------------------------------------------------------------
// Tiền tệ + kim loại quý (vàng/bạc) — tỉ giá LIVE, đổi ở backend (currency_convert).
// Ở đây chỉ NHẬN DIỆN truy vấn tiền tệ và trả {amount, from, to} (mã ISO / XAU / XAG).
// ---------------------------------------------------------------------------
const CURRENCY_ALIAS: Record<string, string> = {
  "$": "USD", usd: "USD", dollar: "USD", "đô": "USD", "đôla": "USD", "đô la": "USD", "dola": "USD",
  "₫": "VND", vnd: "VND", "vnđ": "VND", "đồng": "VND", dong: "VND",
  ngn: "NGN", naira: "NGN",
  "€": "EUR", eur: "EUR", euro: "EUR",
  "£": "GBP", gbp: "GBP",
  "¥": "JPY", jpy: "JPY", yen: "JPY", "yên": "JPY",
  cny: "CNY", yuan: "CNY", rmb: "CNY", "tệ": "CNY", "nhân dân tệ": "CNY",
  krw: "KRW", won: "KRW",
  thb: "THB", baht: "THB",
  rub: "RUB", ruble: "RUB", inr: "INR", rupee: "INR",
  gold: "XAU", "vàng": "XAU", vang: "XAU", xau: "XAU",
  silver: "XAG", "bạc": "XAG", bac: "XAG", xag: "XAG",
  // Đơn vị vàng VN: 1 cây (lượng) = 37.5g, 1 chỉ = 3.75g
  "cây": "CAY", cay: "CAY", "lượng": "CAY", luong: "CAY", "lạng": "CAY", cayvang: "CAY",
  "chỉ": "CHI", chi: "CHI", chivang: "CHI",
};
const ISO_CURRENCIES = new Set([
  "USD", "EUR", "JPY", "GBP", "AUD", "CAD", "CHF", "CNY", "HKD", "NZD", "SEK", "KRW",
  "SGD", "NOK", "MXN", "INR", "RUB", "ZAR", "TRY", "BRL", "TWD", "DKK", "PLN", "THB",
  "IDR", "HUF", "CZK", "ILS", "CLP", "PHP", "AED", "COP", "SAR", "MYR", "RON", "VND",
  "NGN", "EGP", "PKR", "BDT", "UAH", "KES", "GHS", "MAD", "QAR", "KWD", "BHD", "OMR",
  "JOD", "LKR", "MMK", "KHR", "LAK", "XAU", "XAG", "CAY", "CHI",
]);

/** Gộp "cây vàng"/"chỉ vàng" -> "cây"/"chỉ" để token khớp 1 từ. */
function collapseGold(q: string): string {
  return q.replace(/(cây|chỉ|lượng|lạng|cay|chi|luong)\s+vàng/giu, "$1");
}

function normCurrency(tok: string): string | null {
  const low = tok.trim().toLowerCase();
  if (CURRENCY_ALIAS[low]) return CURRENCY_ALIAS[low];
  const up = tok.trim().toUpperCase();
  return ISO_CURRENCIES.has(up) ? up : null;
}

/** Token có phải đơn vị vật lý đã biết không (dùng cho thông báo lỗi). */
function isKnownUnit(tok: string): boolean {
  const k = normalize(tok);
  return !!KEY_CAT[k];
}

/**
 * Nếu truy vấn CÓ DẠNG đổi "số <a> to <b>" nhưng không đổi được, trả thông báo
 * lỗi (chỉ rõ token nào không nhận diện). Trả null nếu không phải dạng đổi.
 */
export function convError(q: string): string | null {
  const m = /^(-?[\d.,]+(?:[kmb](?=\s))?)\s*([\p{L}$€£¥₫²³/'"]+)\s+(?:to|sang|ra|thành|=|->|→|in)\s+([\p{L}$€£¥₫²³/'"]+)$/iu.exec(collapseGold(q.trim()));
  if (!m) return null;
  const a = m[2], b = m[3];
  const known = (t: string) => !!normCurrency(t) || isKnownUnit(t);
  const bad: string[] = [];
  if (!known(a)) bad.push(a);
  if (!known(b)) bad.push(b);
  if (bad.length) return `Không nhận diện đơn vị / mã tiền tệ: ${bad.join(", ")}`;
  // cả hai đều biết nhưng khác bản chất (vd tiền ↔ chiều dài)
  return `Không đổi được "${a}" → "${b}" — khác bản chất`;
}

export function parseCurrency(q: string): { amount: number; from: string; to: string } | null {
  const m = /^([\d.,]+(?:[kmb](?=\s))?)\s*([\p{L}$€£¥₫]+)\s+(?:to|sang|ra|thành|=|->|→)\s+([\p{L}$€£¥₫]+)$/iu.exec(collapseGold(q.trim()));
  if (!m) return null;
  const amount = parseNumber(m[1]);
  if (!isFinite(amount)) return null;
  const from = normCurrency(m[2]);
  const to = normCurrency(m[3]);
  if (!from || !to || from === to) return null;
  return { amount, from, to };
}

type DataBase = "bin" | "dec" | "hex" | "ascii";

function dataBase(raw: string): DataBase | null {
  const v = raw.toLowerCase();
  if (["bin", "binary", "base2"].includes(v)) return "bin";
  if (["dec", "decimal", "base10"].includes(v)) return "dec";
  if (["hex", "hexadecimal", "base16"].includes(v)) return "hex";
  if (["ascii", "text", "string"].includes(v)) return "ascii";
  return null;
}

function parseBytes(value: string, from: Exclude<DataBase, "ascii">): number[] | null {
  const cleaned = value.trim().replace(/0x/gi, "").replace(/0b/gi, "");
  const tokens = cleaned.split(/[\s,]+/).filter(Boolean);
  const radix = from === "bin" ? 2 : from === "hex" ? 16 : 10;
  const valid = from === "bin" ? /^[01]+$/ : from === "hex" ? /^[0-9a-f]+$/i : /^\d+$/;
  if (!tokens.length || tokens.some((t) => !valid.test(t))) return null;
  const bytes = tokens.map((t) => parseInt(t, radix));
  return bytes.every((b) => Number.isInteger(b) && b >= 0 && b <= 255) ? bytes : null;
}

function tryDataConvert(raw: string): ResultItemData | null {
  const q = raw.trim();
  const names = "bin|binary|base2|dec|decimal|base10|hex|hexadecimal|base16|ascii|text|string";
  let fromRaw = "", toRaw = "", value = "";
  let m = new RegExp(`^(${names})\\s+(.+?)\\s+(?:to|sang|ra|=)\\s+(${names})$`, "i").exec(q);
  if (m) [, fromRaw, value, toRaw] = m;
  else {
    m = new RegExp(`^(.+?)\\s+(${names})\\s+(?:to|sang|ra|=)\\s+(${names})$`, "i").exec(q);
    if (m) [, value, fromRaw, toRaw] = m;
  }
  if (!m) {
    const p = new RegExp(`^(0x[0-9a-f]+|0b[01]+)\\s+(?:to|sang|ra|=)\\s+(${names})$`, "i").exec(q);
    if (!p) return null;
    value = p[1]; fromRaw = /^0x/i.test(value) ? "hex" : "bin"; toRaw = p[2];
  }
  const from = dataBase(fromRaw), to = dataBase(toRaw);
  if (!from || !to || from === to || !value) return null;

  let bytes = from === "ascii" ? Array.from(new TextEncoder().encode(value)) : parseBytes(value, from);
  if (!bytes && from !== "ascii" && to !== "ascii" && !/[\s,]/.test(value.trim())) {
    const normalized = value.replace(/^0x|^0b/i, "");
    const valid = from === "bin" ? /^[01]+$/ : from === "hex" ? /^[0-9a-f]+$/i : /^\d+$/;
    if (!valid.test(normalized)) return null;
    try {
      const n = from === "bin" ? BigInt(`0b${normalized}`) : from === "hex" ? BigInt(`0x${normalized}`) : BigInt(normalized);
      const result = to === "bin" ? n.toString(2) : to === "hex" ? n.toString(16).toUpperCase() : n.toString(10);
      return { id: `data:${q}`, title: result, subtitle: `${from.toUpperCase()} → ${to.toUpperCase()} — Enter để copy`, kind: "unit", text: result };
    } catch { return null; }
  }
  if (!bytes) return null;
  const result = to === "ascii" ? new TextDecoder().decode(Uint8Array.from(bytes))
    : to === "hex" ? bytes.map((b) => b.toString(16).toUpperCase().padStart(2, "0")).join(" ")
    : to === "bin" ? bytes.map((b) => b.toString(2).padStart(8, "0")).join(" ")
    : bytes.join(" ");
  return { id: `data:${q}`, title: result, subtitle: `${from.toUpperCase()} → ${to.toUpperCase()} — Enter để copy`, kind: "unit", text: result };
}
