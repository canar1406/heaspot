import type { ResultItemData } from "../types";

interface Engine {
  name: string;
  build: (q: string) => string;
}

const ENGINES: Record<string, Engine> = {
  g: {
    name: "Google",
    build: (q) => `https://www.google.com/search?q=${encodeURIComponent(q)}`,
  },
  yt: {
    name: "YouTube",
    build: (q) =>
      `https://www.youtube.com/results?search_query=${encodeURIComponent(q)}`,
  },
  wiki: {
    name: "Wikipedia",
    build: (q) =>
      `https://vi.wikipedia.org/wiki/Special:Search?search=${encodeURIComponent(q)}`,
  },
};

/** Bắt cú pháp "g <từ khoá>", "yt <từ khoá>", "wiki <từ khoá>" */
export function tryWebSearch(input: string): ResultItemData | null {
  const m = /^(\S+)\s+(.+)$/.exec(input.trim());
  if (!m) return null;
  const engine = ENGINES[m[1].toLowerCase()];
  if (!engine) return null;
  const q = m[2].trim();
  return {
    id: `web:${m[1]}:${q}`,
    title: `Tìm "${q}" trên ${engine.name}`,
    subtitle: engine.build(q),
    kind: "web",
    url: engine.build(q),
  };
}

/** Kết quả fallback cuối danh sách: tìm nguyên câu trên Google */
export function googleFallback(query: string): ResultItemData {
  return {
    id: `web:fallback:${query}`,
    title: `Tìm "${query}" trên Google`,
    subtitle: "Web Search",
    kind: "web",
    url: ENGINES.g.build(query),
  };
}
