/**
 * Dev tools frontend: format JSON (json) và decode JWT (jwt).
 */
import type { ResultItemData } from "../types";

/** Format / kiểm tra JSON. `text` thường lấy từ clipboard. */
export function buildJsonResults(text: string): ResultItemData[] {
  const src = text.trim();
  if (!src) {
    return [{
      id: "json:empty", title: "JSON — copy JSON rồi gõ `json`",
      subtitle: "Đọc clipboard, format đẹp / minify / kiểm tra lỗi",
      kind: "knowledge", action: "json", text: "",
      preview: "Copy một đoạn JSON vào clipboard, rồi gõ `json` để format & kiểm tra.",
    }];
  }
  try {
    const parsed = JSON.parse(src);
    const pretty = JSON.stringify(parsed, null, 2);
    const mini = JSON.stringify(parsed);
    const keys = typeof parsed === "object" && parsed ? Object.keys(parsed).length : 0;
    return [
      {
        id: "json:pretty", title: "JSON hợp lệ ✓ — format đẹp",
        subtitle: `${pretty.split("\n").length} dòng · ${keys} khóa gốc · Enter để copy`,
        kind: "knowledge", action: "json", text: pretty, preview: pretty,
      },
      {
        id: "json:mini", title: "Copy JSON minified",
        subtitle: `${mini.length} ký tự`, kind: "generated", text: mini,
      },
    ];
  } catch (e) {
    return [{
      id: "json:error", title: "JSON không hợp lệ ✗",
      subtitle: String(e instanceof Error ? e.message : e),
      kind: "knowledge", action: "json", text: src,
      preview: `Lỗi: ${e instanceof Error ? e.message : e}\n\n--- Nội dung ---\n${src.slice(0, 4000)}`,
    }];
  }
}

function b64urlDecode(s: string): string {
  let t = s.replace(/-/g, "+").replace(/_/g, "/");
  const pad = t.length % 4;
  if (pad) t += "=".repeat(4 - pad);
  try {
    return decodeURIComponent(
      Array.from(atob(t))
        .map((c) => "%" + c.charCodeAt(0).toString(16).padStart(2, "0"))
        .join("")
    );
  } catch {
    return atob(t);
  }
}

function fmtDate(sec: number): string {
  return new Date(sec * 1000).toLocaleString("vi-VN");
}

/** Decode JWT token: header + payload + exp/iat dạng ngày. Không verify chữ ký. */
export function buildJwtResults(token: string): ResultItemData[] {
  const t = token.trim();
  if (!t) {
    return [{
      id: "jwt:empty", title: "JWT — gõ `jwt <token>`",
      subtitle: "Giải mã header & payload (không verify chữ ký)",
      kind: "knowledge", action: "jwt", text: "",
      preview: "Dán token: jwt eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    }];
  }
  const parts = t.split(".");
  if (parts.length < 2) {
    return [{
      id: "jwt:invalid", title: "Token JWT không hợp lệ ✗",
      subtitle: "JWT phải có dạng header.payload.signature",
      kind: "knowledge", action: "jwt", text: "", preview: "Token không đúng định dạng JWT.",
    }];
  }
  try {
    const header = JSON.parse(b64urlDecode(parts[0]));
    const payload = JSON.parse(b64urlDecode(parts[1]));
    const meta: string[] = [];
    if (payload.exp) {
      const expired = payload.exp * 1000 < Date.now();
      meta.push(`exp: ${fmtDate(payload.exp)} ${expired ? "⚠ ĐÃ HẾT HẠN" : "✓ còn hạn"}`);
    }
    if (payload.iat) meta.push(`iat: ${fmtDate(payload.iat)}`);
    if (payload.nbf) meta.push(`nbf: ${fmtDate(payload.nbf)}`);
    const preview = [
      `alg: ${header.alg || "?"} · typ: ${header.typ || "?"}`,
      meta.join("\n"),
      "--- HEADER ---",
      JSON.stringify(header, null, 2),
      "--- PAYLOAD ---",
      JSON.stringify(payload, null, 2),
    ].filter(Boolean).join("\n");
    return [{
      id: "jwt:decoded",
      title: payload.exp && payload.exp * 1000 < Date.now() ? "JWT ⚠ đã hết hạn" : "JWT đã giải mã ✓",
      subtitle: `${header.alg || "?"} · ${Object.keys(payload).length} claim · Enter copy payload`,
      kind: "knowledge", action: "jwt", text: JSON.stringify(payload, null, 2), preview,
    }];
  } catch (e) {
    return [{
      id: "jwt:error", title: "Không giải mã được JWT ✗",
      subtitle: String(e instanceof Error ? e.message : e),
      kind: "knowledge", action: "jwt", text: "", preview: String(e),
    }];
  }
}
