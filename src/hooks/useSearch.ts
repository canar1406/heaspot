import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { tryCalculate } from "../plugins/calculator";
import { tryConvert } from "../plugins/converter";
import { tryTime } from "../plugins/timezones";
import { tryUrl } from "../plugins/urlDetect";
import { generatorHelp, parseGenerator } from "../plugins/generator";
import { googleFallback, tryWebSearch } from "../plugins/websearch";
import { matchSystemCommands } from "../plugins/systemCommands";
import type {
  BackendSearchResponse,
  CapacitiesHit,
  FullTextHit,
  OpenWindowInfo,
  RegKeyInfo,
  ResultItemData,
  ServiceInfo,
  Snippet,
  VsCodeEntry,
  WorkflowInfo,
  WorkflowItem,
} from "../types";

/** Keyword đã bị hệ thống dùng — workflow Alfred không được đè lên */
const RESERVED_KEYWORDS = new Set([
  "g", "yt", "wiki", "in", "snip", "time", "date", "capacities", "workflow",
]);

/**
 * Router tổng hợp kết quả (triết lý Trigger Keyword — plan3):
 * `<` window walker · `{` VS Code · `!` services · `:` registry · `#` generator
 * `>` terminal · `;` snippets · `in` full-text (Windows Search + Capacities)
 * còn lại: calc, unit, time, url, web, system command, app/file search, workflows.
 */
export function useSearch(query: string, refreshKey: number) {
  const [results, setResults] = useState<ResultItemData[]>([]);
  const [engine, setEngine] = useState<string>("internal");
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [workflows, setWorkflows] = useState<WorkflowInfo[]>([]);
  const [hasCapToken, setHasCapToken] = useState(false);
  const seq = useRef(0);

  // Nạp snippets / workflows / trạng thái Capacities mỗi lần mở cửa sổ
  useEffect(() => {
    invoke<Snippet[]>("get_snippets")
      .then(setSnippets)
      .catch(() => setSnippets([]));
    invoke<WorkflowInfo[]>("list_workflows")
      .then(setWorkflows)
      .catch(() => setWorkflows([]));
    invoke<boolean>("capacities_has_token")
      .then(setHasCapToken)
      .catch(() => setHasCapToken(false));
  }, [refreshKey]);

  useEffect(() => {
    const q = query.trim();
    const mySeq = ++seq.current;
    const fresh = (fn: () => void) => {
      if (seq.current === mySeq) fn();
    };

    if (!q) {
      setResults([]);
      return;
    }

    // ">" — Terminal mode
    if (q.startsWith(">")) {
      const cmd = q.slice(1).trim();
      setResults(
        cmd
          ? [
              {
                id: "terminal",
                title: cmd,
                subtitle: "Chạy trong cửa sổ Terminal mới — Enter (→ để chọn thêm)",
                kind: "terminal",
                text: cmd,
              },
            ]
          : []
      );
      return;
    }

    // "<" — Window Walker: chuyển focus cửa sổ đang mở
    if (q.startsWith("<")) {
      const filter = q.slice(1).trim();
      const t = setTimeout(async () => {
        try {
          const wins = await invoke<OpenWindowInfo[]>("list_windows", { query: filter });
          fresh(() =>
            setResults(
              wins.map((w) => ({
                id: `win:${w.hwnd}`,
                title: w.title,
                subtitle: w.process,
                kind: "window" as const,
                hwnd: w.hwnd,
                icon: w.icon ?? undefined,
              }))
            )
          );
        } catch {
          fresh(() => setResults([]));
        }
      }, 80);
      return () => clearTimeout(t);
    }

    // "{" — VS Code recent workspaces
    if (q.startsWith("{")) {
      const filter = q.slice(1).trim();
      const t = setTimeout(async () => {
        try {
          const list = await invoke<VsCodeEntry[]>("vscode_recent", { query: filter });
          fresh(() =>
            setResults(
              list.map((e) => ({
                id: `vsc:${e.path}`,
                title: e.name,
                subtitle: e.path,
                kind: "vscode" as const,
                path: e.path,
              }))
            )
          );
        } catch {
          fresh(() => setResults([]));
        }
      }, 80);
      return () => clearTimeout(t);
    }

    // "!" — Windows Services
    if (q.startsWith("!")) {
      const filter = q.slice(1).trim();
      const t = setTimeout(async () => {
        try {
          const list = await invoke<ServiceInfo[]>("list_services", { query: filter });
          fresh(() =>
            setResults(
              list.map((s) => ({
                id: `svc:${s.name}`,
                title: s.display || s.name,
                subtitle: `${s.status} · ${s.name} — → để Start/Stop/Restart`,
                kind: "service" as const,
                text: s.name,
              }))
            )
          );
        } catch {
          fresh(() => setResults([]));
        }
      }, 300);
      return () => clearTimeout(t);
    }

    // ":" — Registry browser
    if (q.startsWith(":")) {
      const filter = q.slice(1).trim();
      const t = setTimeout(async () => {
        try {
          const list = await invoke<RegKeyInfo[]>("registry_search", { query: filter });
          fresh(() =>
            setResults(
              list.map((k) => ({
                id: `reg:${k.path}`,
                title: k.name,
                subtitle: k.path,
                kind: "registry" as const,
                path: k.path,
              }))
            )
          );
        } catch {
          fresh(() => setResults([]));
        }
      }, 120);
      return () => clearTimeout(t);
    }

    // "#" — Value Generator (uuid, hash, base64)
    if (q.startsWith("#")) {
      const req = parseGenerator(q);
      if (!req) {
        setResults([]);
        return;
      }
      if (req.type === "immediate") {
        setResults([req.item]);
        return;
      }
      if (req.type === "help") {
        setResults(generatorHelp());
        return;
      }
      const { algo, text } = req;
      const t = setTimeout(async () => {
        try {
          const hash = await invoke<string>("hash_text", { algo, text });
          fresh(() =>
            setResults([
              {
                id: `gen:${algo}:${text}`,
                title: hash,
                subtitle: `${algo.toUpperCase()}("${text}") — Enter để copy`,
                kind: "generated",
                text: hash,
              },
            ])
          );
        } catch {
          fresh(() => setResults([]));
        }
      }, 150);
      return () => clearTimeout(t);
    }

    // ";" — Snippets (Text Expander)
    if (q.startsWith(";")) {
      const key = q.slice(1).trim().toLowerCase();
      const list: ResultItemData[] = snippets
        .filter(
          (s) =>
            !key ||
            s.keyword.toLowerCase().includes(key) ||
            s.content.toLowerCase().includes(key)
        )
        .map((s) => ({
          id: `snip:${s.id}`,
          title: `;${s.keyword}`,
          subtitle: s.content.length > 90 ? s.content.slice(0, 90) + "…" : s.content,
          kind: "snippet" as const,
          text: s.content,
        }));
      setResults(
        list.length
          ? list
          : [
              {
                id: "snip:empty",
                title: "Chưa có snippet nào khớp",
                subtitle: "Thêm mới bằng cú pháp: snip <keyword> <nội dung>",
                kind: "snippet",
                text: "",
              },
            ]
      );
      return;
    }

    // "snip <keyword> <nội dung>" — thêm snippet
    const snipAdd = /^snip\s+(\S+)\s+(.+)$/.exec(q);
    if (snipAdd) {
      setResults([
        {
          id: "snip:add",
          title: `Lưu snippet ";${snipAdd[1]}"`,
          subtitle: snipAdd[2],
          kind: "snippet-add",
          keyword: snipAdd[1],
          text: snipAdd[2],
        },
      ]);
      return;
    }

    // "capacities token <token>" — lưu API token của Capacities
    const capToken = /^capacities\s+token\s+(\S+)\s*$/i.exec(q);
    if (capToken) {
      setResults([
        {
          id: "cap:save-token",
          title: "Lưu Capacities API token",
          subtitle: "Enter để lưu — sau đó gõ `in <từ khoá>` là tìm được nội dung note",
          kind: "cap-token",
          text: capToken[1],
        },
      ]);
      return;
    }

    // "workflow install <path>" / "workflow rescan" — quản lý Alfred workflows
    const wfInstall = /^workflow\s+install\s+(.+)$/i.exec(q);
    if (wfInstall) {
      setResults([
        {
          id: "wf:install",
          title: "Cài Alfred Workflow",
          subtitle: wfInstall[1],
          kind: "wf-cmd",
          action: "install",
          text: wfInstall[1],
        },
      ]);
      return;
    }
    if (/^workflow(\s+rescan)?$/i.test(q)) {
      const list: ResultItemData[] = workflows.map((w) => ({
        id: `wf:${w.keyword}`,
        title: `${w.keyword} — ${w.name}`,
        subtitle: `Alfred Workflow (${w.script_type}) · gõ "${w.keyword} <từ khoá>" để dùng`,
        kind: "workflow" as const,
        icon: w.icon ?? undefined,
        text: "",
      }));
      list.push({
        id: "wf:rescan",
        title: "Quét lại thư mục workflows",
        subtitle: "workflow install <đường dẫn .alfredworkflow> để cài mới",
        kind: "wf-cmd",
        action: "rescan",
        text: "",
      });
      setResults(list);
      return;
    }

    // Alfred workflow keyword: "<keyword> <query>"
    const firstToken = q.split(/\s+/)[0].toLowerCase();
    const wf = !RESERVED_KEYWORDS.has(firstToken)
      ? workflows.find((w) => w.keyword.toLowerCase() === firstToken)
      : undefined;
    if (wf) {
      const rest = q.slice(firstToken.length).trim();
      const t = setTimeout(async () => {
        try {
          const items = await invoke<WorkflowItem[]>("run_workflow", {
            keyword: wf.keyword,
            query: rest,
          });
          fresh(() =>
            setResults(
              items.length
                ? items.map((it, i) => ({
                    id: `wfi:${i}:${it.arg}`,
                    title: it.title,
                    subtitle: it.subtitle || wf.name,
                    kind: "workflow" as const,
                    icon: wf.icon ?? undefined,
                    text: it.arg,
                  }))
                : [
                    {
                      id: "wfi:empty",
                      title: `${wf.name}: không có kết quả`,
                      subtitle: "workflow không trả về item nào",
                      kind: "workflow",
                      text: "",
                    },
                  ]
            )
          );
        } catch (err) {
          fresh(() =>
            setResults([
              {
                id: "wfi:error",
                title: `${wf.name}: lỗi khi chạy workflow`,
                subtitle: String(err),
                kind: "workflow",
                text: "",
              },
            ])
          );
        }
      }, 300);
      return () => clearTimeout(t);
    }

    // "in <từ khoá>" / "doc:<từ khoá>" — Windows Search + Capacities
    const ft = /^(?:in\s+|doc:\s*)(.+)$/.exec(q);
    if (ft) {
      const kw = ft[1].trim();
      const t = setTimeout(async () => {
        const [ftRes, capRes] = await Promise.allSettled([
          invoke<FullTextHit[]>("fulltext_search", { query: kw }),
          invoke<CapacitiesHit[]>("capacities_search", { query: kw }),
        ]);
        if (seq.current !== mySeq) return;

        const list: ResultItemData[] = [];
        if (capRes.status === "fulfilled") {
          list.push(
            ...capRes.value.map((h) => ({
              id: `cap:${h.id}`,
              title: h.title || "(không tiêu đề)",
              subtitle: h.preview ? `Capacities · ${h.preview}` : "Capacities",
              kind: "capacities" as const,
              url: `capacities://${h.space_id}/${h.id}`,
            }))
          );
        }
        if (ftRes.status === "fulfilled") {
          list.push(
            ...ftRes.value.map((h) => ({
              id: `ft:${h.path}`,
              title: h.name,
              subtitle: h.preview || h.path,
              kind: "fulltext" as const,
              path: h.path,
            }))
          );
        }
        if (!hasCapToken) {
          list.push({
            id: "cap:hint",
            title: "Kết nối Capacities để tìm trong note",
            subtitle:
              "Capacities → Settings → Capacities API → tạo token, rồi gõ: capacities token <token>",
            kind: "cap-token",
            text: "",
          });
        }
        if (list.length === 0) {
          list.push({
            id: "ft:empty",
            title: `Không tìm thấy nội dung "${kw}"`,
            subtitle: "Windows Search Index chỉ quét các thư mục đã được index",
            kind: "fulltext",
            path: "",
          });
        }
        setResults(list);
      }, 350);
      return () => clearTimeout(t);
    }

    // Mặc định: calc / unit / time / url / web / system hiện ngay,
    // app + file từ backend đổ về sau (debounce ~90ms)
    const immediate: ResultItemData[] = [];
    const calc = tryCalculate(q);
    if (calc !== null) {
      immediate.push({
        id: "calc",
        title: `= ${calc}`,
        subtitle: `${q} — Enter để copy kết quả`,
        kind: "calc",
        text: calc,
      });
    }
    const unit = tryConvert(q);
    if (unit) immediate.push(unit);
    const time = tryTime(q);
    if (time) immediate.push(time);
    const url = tryUrl(q);
    if (url) immediate.push(url);
    const web = tryWebSearch(q);
    if (web) immediate.push(web);
    immediate.push(...matchSystemCommands(q));

    setResults((prev) => [
      ...immediate,
      ...prev.filter((p) => ["app", "file", "folder"].includes(p.kind)),
    ]);

    const t = setTimeout(async () => {
      try {
        const resp = await invoke<BackendSearchResponse>("search_all", { query: q });
        if (seq.current !== mySeq) return;
        setEngine(resp.engine);
        const backend: ResultItemData[] = resp.results.map((r) => ({
          id: `${r.kind}:${r.path}`,
          title: r.title,
          subtitle: r.subtitle,
          kind: r.kind,
          path: r.path,
          icon: r.icon ?? undefined,
        }));
        const all = [...immediate, ...backend];
        if (!web && calc === null && !url) all.push(googleFallback(q));
        setResults(all);
      } catch {
        fresh(() => setResults(immediate));
      }
    }, 90);
    return () => clearTimeout(t);
  }, [query, snippets, workflows, hasCapToken]);

  return { results, engine };
}
