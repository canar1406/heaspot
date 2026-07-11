/**
 * Unit Converter: "10 ft to m", "5 kg sang lb", "100 f to c", "2 gb to mb"
 */
import type { ResultItemData } from "../types";

const ALIAS: Record<string, string> = {
  meter: "m", meters: "m", metre: "m",
  kilometer: "km", kilometers: "km",
  centimeter: "cm", millimeter: "mm",
  mile: "mi", miles: "mi",
  feet: "ft", foot: "ft",
  inch: "in", inches: "in",
  yard: "yd", yards: "yd",
  kilogram: "kg", kilograms: "kg", kilo: "kg",
  gram: "g", grams: "g", milligram: "mg",
  ton: "t", tonne: "t", tan: "t", "tấn": "t",
  pound: "lb", pounds: "lb", lbs: "lb",
  ounce: "oz",
  second: "s", seconds: "s", sec: "s", giay: "s", "giây": "s",
  minute: "min", minutes: "min", phut: "min", "phút": "min",
  hour: "h", hours: "h", hr: "h", gio: "h", "giờ": "h",
  days: "day", ngay: "day", "ngày": "day",
  weeks: "week", tuan: "week", "tuần": "week",
  byte: "b", bytes: "b",
  celsius: "c", "°c": "c",
  fahrenheit: "f", "°f": "f",
  kelvin: "k",
};

const TABLES: Record<string, Record<string, number>> = {
  length: { m: 1, km: 1000, cm: 0.01, mm: 0.001, mi: 1609.344, ft: 0.3048, in: 0.0254, yd: 0.9144 },
  mass: { kg: 1, g: 0.001, mg: 1e-6, t: 1000, lb: 0.45359237, oz: 0.028349523 },
  time: { s: 1, min: 60, h: 3600, day: 86400, week: 604800 },
  data: { b: 1, kb: 1024, mb: 1024 ** 2, gb: 1024 ** 3, tb: 1024 ** 4 },
};

function normalize(u: string): string {
  const low = u.toLowerCase();
  return ALIAS[low] ?? low;
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
  const r = parseFloat(v.toPrecision(8));
  return Math.abs(r) >= 1e15 ? r.toExponential(4) : r.toLocaleString("en-US");
}

export function tryConvert(q: string): ResultItemData | null {
  const data = tryDataConvert(q);
  if (data) return data;

  const m = /^([\d.,]+)\s*([a-zA-Z°µ"']+)\s+(?:to|sang|ra|=)\s+([a-zA-Z°µ"']+)$/i.exec(q.trim());
  if (!m) return null;
  const value = parseFloat(m[1].replace(/,/g, ""));
  if (!isFinite(value)) return null;
  const from = normalize(m[2]);
  const to = normalize(m[3]);

  let out: number | null = null;
  if (["c", "f", "k"].includes(from) && ["c", "f", "k"].includes(to)) {
    out = convertTemp(value, from, to);
  } else {
    for (const table of Object.values(TABLES)) {
      if (from in table && to in table) {
        out = (value * table[from]) / table[to];
        break;
      }
    }
  }
  if (out == null) return null;

  const result = `${fmt(out)} ${to}`;
  return {
    id: `unit:${q}`,
    title: `${m[1]} ${from} = ${result}`,
    subtitle: "Unit Converter — Enter để copy",
    kind: "unit",
    text: fmt(out),
  };
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
