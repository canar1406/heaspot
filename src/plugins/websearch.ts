import type { ResultItemData } from "../types";

export interface WebEngineKeywords {
  google: string;
  youtube: string;
}

/** Bắt cú pháp "<google> <từ khoá>" / "<youtube> <từ khoá>" theo keyword tùy chỉnh */
export function tryWebSearch(input: string, kw: WebEngineKeywords): ResultItemData | null {
  const m = /^(\S+)\s+(.+)$/.exec(input.trim());
  if (!m) return null;
  const token = m[1].toLowerCase();
  const q = m[2].trim();

  let name = "";
  let url = "";
  if (token === kw.google.toLowerCase()) {
    name = "Google";
    url = `https://www.google.com/search?q=${encodeURIComponent(q)}`;
  } else if (token === kw.youtube.toLowerCase()) {
    name = "YouTube";
    url = `https://www.youtube.com/results?search_query=${encodeURIComponent(q)}`;
  } else {
    return null;
  }
  return {
    id: `web:${token}:${q}`,
    title: `Tìm "${q}" trên ${name}`,
    subtitle: url,
    kind: "web",
    url,
  };
}
