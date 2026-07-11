/**
 * URI/URL Handler: gõ "github.com", "https://...", "mailto:a@b.com" -> mở ngay
 */
import type { ResultItemData } from "../types";

const COMMON_TLDS = new Set([
  "com", "net", "org", "io", "dev", "app", "ai", "co", "me", "vn", "edu",
  "gov", "info", "xyz", "gg", "tv", "so", "sh", "to", "uk", "jp", "kr",
  "de", "fr", "ru", "br", "us", "ca", "au", "in", "id", "sg", "th",
]);

function make(url: string, display: string): ResultItemData {
  return {
    id: `url:${url}`,
    title: `Mở ${display}`,
    subtitle: url,
    kind: "url",
    url,
  };
}

export function tryUrl(q: string): ResultItemData | null {
  const s = q.trim();
  if (!s || /\s/.test(s)) return null;

  if (/^https?:\/\/\S+$/i.test(s)) return make(s, s);
  if (/^mailto:\S+@\S+$/i.test(s)) return make(s, s);
  if (/^\S+@\S+\.\S+$/.test(s)) return make(`mailto:${s}`, s);

  // Bare domain: chỉ nhận TLD phổ biến để không nhầm với tên file (plan.md...)
  const domain = /^([\w-]+(?:\.[\w-]+)+)(\/\S*)?$/.exec(s);
  if (domain) {
    const labels = domain[1].split(".");
    const tld = labels[labels.length - 1].toLowerCase();
    if (COMMON_TLDS.has(tld)) return make(`https://${s}`, s);
  }
  return null;
}
