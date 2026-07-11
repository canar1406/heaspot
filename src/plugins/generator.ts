/**
 * Value Generator (`#`): "# uuid", "# md5 hello", "# sha256 abc",
 * "# b64 hello" (encode), "# b64d aGVsbG8=" (decode)
 */
import type { ResultItemData } from "../types";

export type GeneratorRequest =
  | { type: "immediate"; item: ResultItemData }
  | { type: "hash"; algo: string; text: string }
  | { type: "help" };

function item(title: string, subtitle: string, text: string): ResultItemData {
  return { id: `gen:${subtitle}:${text}`, title, subtitle, kind: "generated", text };
}

export function parseGenerator(q: string): GeneratorRequest | null {
  if (!q.startsWith("#")) return null;
  const rest = q.slice(1).trim();

  if (!rest || rest === "uuid" || rest === "guid") {
    const uuid = crypto.randomUUID();
    return rest
      ? { type: "immediate", item: item(uuid, "UUID v4 — Enter để copy", uuid) }
      : { type: "help" };
  }

  const b64 = /^(b64|base64)\s+(.+)$/i.exec(rest);
  if (b64) {
    try {
      const encoded = btoa(String.fromCharCode(...new TextEncoder().encode(b64[2])));
      return { type: "immediate", item: item(encoded, "Base64 encode — Enter để copy", encoded) };
    } catch {
      return null;
    }
  }

  const b64d = /^(b64d|base64d|decode)\s+(.+)$/i.exec(rest);
  if (b64d) {
    try {
      const decoded = new TextDecoder().decode(
        Uint8Array.from(atob(b64d[2]), (c) => c.charCodeAt(0))
      );
      return { type: "immediate", item: item(decoded, "Base64 decode — Enter để copy", decoded) };
    } catch {
      return null;
    }
  }

  const hash = /^(md5|sha1|sha256)\s+(.+)$/i.exec(rest);
  if (hash) {
    return { type: "hash", algo: hash[1].toLowerCase(), text: hash[2] };
  }

  return { type: "help" };
}

export function generatorHelp(): ResultItemData[] {
  const uuid = crypto.randomUUID();
  return [
    item(uuid, "# uuid — UUID v4 mới, Enter để copy", uuid),
    {
      id: "gen:help",
      title: "# md5|sha1|sha256 <text> · # b64|b64d <text>",
      subtitle: "Value Generator — hash & base64",
      kind: "generated",
      text: "",
    },
  ];
}
