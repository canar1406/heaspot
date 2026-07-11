import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { tryCalculate } from "../plugins/calculator";
import { tryConvert } from "../plugins/converter";
import { tryTime } from "../plugins/timezones";
import { tryUrl } from "../plugins/urlDetect";
import { generatorHelp, parseGenerator } from "../plugins/generator";
import { tryWebSearch } from "../plugins/websearch";
import { matchSystemCommands } from "../plugins/systemCommands";
import { findFormulas } from "../plugins/formulas";
import { findChemistry } from "../plugins/chemistry";
import { matchWord, type KwMap, DEFAULT_KEYWORDS } from "../keywords";
import type {
  BackendSearchResponse,
  BrowserPassword,
  CapacitiesHit,
  FullTextHit,
  KnowledgeHit,
  TranslationHit,
  OpenWindowInfo,
  ProcInfo,
  RegKeyInfo,
  ResultItemData,
  ServiceInfo,
  StudyWord,
  Snippet,
  VsCodeEntry,
  WorkflowInfo,
  WorkflowItem,
} from "../types";

/**
 * Router tổng hợp kết quả — mọi keyword đọc từ `kw` (tùy chỉnh trong Settings).
 * prefix (`<{!:#;>`) và word (`in tr wiki conv time url sys g yt formula review ocr`).
 */
export function useSearch(query: string, refreshKey: number, kw: KwMap = DEFAULT_KEYWORDS) {
  const [results, setResults] = useState<ResultItemData[]>([]);
  const [engine, setEngine] = useState<string>("internal");
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [workflows, setWorkflows] = useState<WorkflowInfo[]>([]);
  const [hasCapToken, setHasCapToken] = useState(false);
  const seq = useRef(0);

  useEffect(() => {
    invoke<Snippet[]>("get_snippets").then(setSnippets).catch(() => setSnippets([]));
    invoke<WorkflowInfo[]>("list_workflows").then(setWorkflows).catch(() => setWorkflows([]));
    invoke<boolean>("capacities_has_token").then(setHasCapToken).catch(() => setHasCapToken(false));
  }, [refreshKey]);

  useEffect(() => {
    const q = query.trim();
    const mySeq = ++seq.current;
    const fresh = (fn: () => void) => {
      if (seq.current === mySeq) fn();
    };
    const startsWith = (p: string) => p && q.startsWith(p);
    const afterPrefix = (p: string) => q.slice(p.length).trim();

    if (!q) {
      setResults([]);
      return;
    }

    // ── Prefix triggers ──────────────────────────────────────────────
    if (startsWith(kw.terminal)) {
      const cmd = afterPrefix(kw.terminal);
      setResults(cmd ? [{
        id: "terminal", title: cmd,
        subtitle: "Chạy trong cửa sổ Terminal mới — Enter (→ để chọn thêm)",
        kind: "terminal", text: cmd,
      }] : []);
      return;
    }

    if (startsWith(kw.window)) {
      const filter = afterPrefix(kw.window);
      const t = setTimeout(async () => {
        try {
          const wins = await invoke<OpenWindowInfo[]>("list_windows", { query: filter });
          fresh(() => setResults(wins.map((w) => ({
            id: `win:${w.hwnd}`, title: w.title, subtitle: w.process,
            kind: "window" as const, hwnd: w.hwnd, icon: w.icon ?? undefined,
          }))));
        } catch { fresh(() => setResults([])); }
      }, 80);
      return () => clearTimeout(t);
    }

    if (startsWith(kw.vscode)) {
      const filter = afterPrefix(kw.vscode);
      const t = setTimeout(async () => {
        try {
          const list = await invoke<VsCodeEntry[]>("vscode_recent", { query: filter });
          fresh(() => setResults(list.map((e) => ({
            id: `vsc:${e.path}`, title: e.name, subtitle: e.path, kind: "vscode" as const, path: e.path,
          }))));
        } catch { fresh(() => setResults([])); }
      }, 80);
      return () => clearTimeout(t);
    }

    if (startsWith(kw.service)) {
      const filter = afterPrefix(kw.service);
      const t = setTimeout(async () => {
        try {
          const list = await invoke<ServiceInfo[]>("list_services", { query: filter });
          fresh(() => setResults(list.map((s) => ({
            id: `svc:${s.name}`, title: s.display || s.name,
            subtitle: `${s.status} · ${s.name} — → để Start/Stop/Restart`,
            kind: "service" as const, text: s.name,
          }))));
        } catch { fresh(() => setResults([])); }
      }, 300);
      return () => clearTimeout(t);
    }

    if (startsWith(kw.registry)) {
      const filter = afterPrefix(kw.registry);
      const t = setTimeout(async () => {
        try {
          const list = await invoke<RegKeyInfo[]>("registry_search", { query: filter });
          fresh(() => setResults(list.map((k) => ({
            id: `reg:${k.path}`, title: k.name, subtitle: k.path, kind: "registry" as const, path: k.path,
          }))));
        } catch { fresh(() => setResults([])); }
      }, 120);
      return () => clearTimeout(t);
    }

    if (startsWith(kw.generator)) {
      const req = parseGenerator("#" + afterPrefix(kw.generator));
      if (!req) { setResults([]); return; }
      if (req.type === "immediate") { setResults([req.item]); return; }
      if (req.type === "help") { setResults(generatorHelp()); return; }
      const { algo, text } = req;
      const t = setTimeout(async () => {
        try {
          const hash = await invoke<string>("hash_text", { algo, text });
          fresh(() => setResults([{
            id: `gen:${algo}:${text}`, title: hash,
            subtitle: `${algo.toUpperCase()}("${text}") — Enter để copy`, kind: "generated", text: hash,
          }]));
        } catch { fresh(() => setResults([])); }
      }, 150);
      return () => clearTimeout(t);
    }

    if (startsWith(kw.snippet)) {
      const key = afterPrefix(kw.snippet).toLowerCase();
      const list: ResultItemData[] = snippets
        .filter((s) => !key || s.keyword.toLowerCase().includes(key) || s.content.toLowerCase().includes(key))
        .map((s) => ({
          id: `snip:${s.id}`, title: `${kw.snippet}${s.keyword}`,
          subtitle: s.content.length > 90 ? s.content.slice(0, 90) + "…" : s.content,
          kind: "snippet" as const, text: s.content,
        }));
      setResults(list.length ? list : [{
        id: "snip:empty", title: "Chưa có snippet nào khớp",
        subtitle: "Thêm mới bằng cú pháp: snip <keyword> <nội dung>", kind: "snippet", text: "",
      }]);
      return;
    }

    // ── Fixed-syntax triggers ────────────────────────────────────────
    const snipAdd = /^snip\s+(\S+)\s+(.+)$/.exec(q);
    if (snipAdd) {
      setResults([{
        id: "snip:add", title: `Lưu snippet "${kw.snippet}${snipAdd[1]}"`,
        subtitle: snipAdd[2], kind: "snippet-add", keyword: snipAdd[1], text: snipAdd[2],
      }]);
      return;
    }

    const capToken = /^capacities\s+token\s+(\S+)\s*$/i.exec(q);
    if (capToken) {
      setResults([{
        id: "cap:save-token", title: "Lưu Capacities API token",
        subtitle: `Enter để lưu — sau đó gõ \`${kw.fulltext} <từ khoá>\` là tìm được nội dung note`,
        kind: "cap-token", text: capToken[1],
      }]);
      return;
    }

    // ── Word triggers ────────────────────────────────────────────────
    const wikiArg = matchWord(q, kw.wiki);
    if (wikiArg) {
      const t = setTimeout(async () => {
        try {
          const hits = await invoke<KnowledgeHit[]>("wikipedia_search", { query: wikiArg });
          fresh(() => setResults(hits.map((h, i) => ({
            id: `knowledge:${i}:${h.title}`, title: h.title,
            subtitle: h.extract.length > 120 ? `${h.extract.slice(0, 120)}…` : h.extract,
            kind: "knowledge" as const, text: h.extract, preview: h.extract, url: h.url,
          }))));
        } catch { fresh(() => setResults([])); }
      }, 260);
      return () => clearTimeout(t);
    }

    const trArg = matchWord(q, kw.translate);
    if (trArg) {
      const t = setTimeout(async () => {
        try {
          const hit = await invoke<TranslationHit>("translate_lookup", { query: trArg });
          const details = hit.entries.map((e, i) => {
            const example = e.example ? `\n   Ví dụ: ${e.example}` : "";
            return `${i + 1}. [${e.part_of_speech || "meaning"}] ${e.definition_en}\n   VI: ${e.definition_vi}${example}`;
          }).join("\n\n");
          const preview = [
            `Bản dịch (${hit.source_language.toUpperCase()} → ${hit.target_language.toUpperCase()}):\n${hit.translation}`,
            hit.phonetic ? `Phát âm: ${hit.phonetic}` : "",
            hit.collocations.length ? `Collocations:\n${hit.collocations.join(" · ")}` : "",
            hit.synonyms.length ? `Đồng nghĩa: ${hit.synonyms.join(", ")}` : "",
            hit.antonyms.length ? `Trái nghĩa: ${hit.antonyms.join(", ")}` : "",
            details ? `Word forms & meanings:\n${details}` : "",
          ].filter(Boolean).join("\n\n");
          fresh(() => setResults([{
            id: `translate:${trArg}`, title: hit.translation,
            subtitle: `${hit.source_language.toUpperCase()} → ${hit.target_language.toUpperCase()} · Enter để dán`,
            kind: "knowledge", text: hit.translation, preview, keyword: trArg,
            audio: hit.audio_url || undefined, action: "translate",
          }]));
        } catch (err) {
          fresh(() => setResults([{
            id: "translate:error", title: "Không dịch được nội dung", subtitle: String(err),
            kind: "knowledge", text: "", preview: String(err), action: "translate",
          }]));
        }
      }, 350);
      return () => clearTimeout(t);
    }

    const reviewArg = matchWord(q, kw.review);
    if (reviewArg !== null) {
      const filter = reviewArg || null;
      const t = setTimeout(async () => {
        try {
          const words = await invoke<StudyWord[]>("get_study_words", { filter });
          fresh(() => setResults(words.map((w) => ({
            id: `study:${w.id}`, title: `${w.word} — ${w.translation}`,
            subtitle: `Danh sách ôn tập · ${w.created_at}`, kind: "knowledge" as const,
            text: w.translation, preview: w.details || `${w.word}: ${w.translation}`,
            keyword: w.word, action: "review",
          }))));
        } catch { fresh(() => setResults([])); }
      }, 80);
      return () => clearTimeout(t);
    }

    const formulaArg = matchWord(q, kw.formula);
    if (formulaArg !== null) {
      setResults(findFormulas(formulaArg));
      return;
    }

    const chemArg = matchWord(q, kw.chemistry);
    if (chemArg !== null) {
      setResults(findChemistry(chemArg));
      return;
    }

    if (/^(?:settings?|cai dat)$/i.test(q)) {
      setResults([{
        id: "settings", title: "Mở HeaSpot Settings",
        subtitle: "Hotkey · Keyword · Clipboard · Privacy · Startup", kind: "settings",
      }]);
      return;
    }

    if (matchWord(q, kw.ocr) === "") {
      setResults([{
        id: "ocr:capture", title: "Chụp vùng màn hình và nhận dạng chữ",
        subtitle: "Enter → chọn vùng → OCR offline bằng Windows (hỗ trợ tiếng Việt nếu đã cài gói)",
        kind: "ocr-cmd",
      }]);
      return;
    }

    // Alfred workflows
    const wfInstall = /^workflow\s+install\s+(.+)$/i.exec(q);
    if (wfInstall) {
      setResults([{
        id: "wf:install", title: "Cài Alfred Workflow", subtitle: wfInstall[1],
        kind: "wf-cmd", action: "install", text: wfInstall[1],
      }]);
      return;
    }
    if (/^workflow(\s+rescan)?$/i.test(q)) {
      const list: ResultItemData[] = workflows.map((w) => ({
        id: `wf:${w.keyword}`, title: `${w.keyword} — ${w.name}`,
        subtitle: `Alfred Workflow (${w.script_type}) · gõ "${w.keyword} <từ khoá>" để dùng`,
        kind: "workflow" as const, icon: w.icon ?? undefined, text: "",
      }));
      list.push({
        id: "wf:rescan", title: "Quét lại thư mục workflows",
        subtitle: "workflow install <đường dẫn .alfredworkflow> để cài mới",
        kind: "wf-cmd", action: "rescan", text: "",
      });
      setResults(list);
      return;
    }

    const reserved = new Set<string>([
      ...Object.values(kw).map((v) => v.toLowerCase()),
      "snip", "capacities", "workflow", "settings", "doc:",
    ]);
    const firstToken = q.split(/\s+/)[0].toLowerCase();
    const wf = !reserved.has(firstToken)
      ? workflows.find((w) => w.keyword.toLowerCase() === firstToken)
      : undefined;
    if (wf) {
      const rest = q.slice(firstToken.length).trim();
      const t = setTimeout(async () => {
        try {
          const items = await invoke<WorkflowItem[]>("run_workflow", { keyword: wf.keyword, query: rest });
          fresh(() => setResults(items.length
            ? items.map((it, i) => ({
                id: `wfi:${i}:${it.arg}`, title: it.title, subtitle: it.subtitle || wf.name,
                kind: "workflow" as const, icon: wf.icon ?? undefined, text: it.arg,
              }))
            : [{ id: "wfi:empty", title: `${wf.name}: không có kết quả`, subtitle: "workflow không trả về item nào", kind: "workflow", text: "" }]));
        } catch (err) {
          fresh(() => setResults([{ id: "wfi:error", title: `${wf.name}: lỗi khi chạy workflow`, subtitle: String(err), kind: "workflow", text: "" }]));
        }
      }, 300);
      return () => clearTimeout(t);
    }

    // Full-text: "<fulltext> <từ khoá>" hoặc "doc:<từ khoá>"
    const ftArg = matchWord(q, kw.fulltext) || (/^doc:\s*(.+)$/i.exec(q)?.[1]?.trim() ?? "");
    if (ftArg) {
      const t = setTimeout(async () => {
        const [ftRes, capRes] = await Promise.allSettled([
          invoke<FullTextHit[]>("fulltext_search", { query: ftArg }),
          invoke<CapacitiesHit[]>("capacities_search", { query: ftArg }),
        ]);
        if (seq.current !== mySeq) return;
        const list: ResultItemData[] = [];
        if (capRes.status === "fulfilled") {
          list.push(...capRes.value.map((h) => ({
            id: `cap:${h.id}`, title: h.title || "(không tiêu đề)",
            subtitle: h.preview ? `Capacities · ${h.preview}` : "Capacities",
            kind: "capacities" as const, url: `capacities://${h.space_id}/${h.id}`,
          })));
        }
        if (ftRes.status === "fulfilled") {
          list.push(...ftRes.value.map((h) => ({
            id: `ft:${h.path}`, title: h.name, subtitle: h.preview || h.path,
            kind: "fulltext" as const, path: h.path,
          })));
        }
        if (!hasCapToken) {
          list.push({
            id: "cap:hint", title: "Kết nối Capacities để tìm trong note",
            subtitle: "Capacities → Settings → Capacities API → tạo token, rồi gõ: capacities token <token>",
            kind: "cap-token", text: "",
          });
        }
        if (list.length === 0) {
          list.push({
            id: "ft:empty", title: `Không tìm thấy nội dung "${ftArg}"`,
            subtitle: "Windows Search Index chỉ quét các thư mục đã được index", kind: "fulltext", path: "",
          });
        }
        setResults(list);
      }, 350);
      return () => clearTimeout(t);
    }

    // Calc ép bằng "="
    if (q.startsWith("=")) {
      const calc = tryCalculate(q);
      setResults(calc === null ? [] : [{
        id: "calc", title: `= ${calc}`, subtitle: `${q} — Enter để copy kết quả`, kind: "calc", text: calc,
      }]);
      return;
    }

    const convArg = matchWord(q, kw.convert);
    if (convArg) {
      const unit = tryConvert(convArg);
      setResults(unit ? [unit] : []);
      return;
    }

    if (matchWord(q, kw.time) !== null) {
      const time = tryTime("time " + afterKeyword(q, kw.time));
      setResults(time ? [time] : []);
      return;
    }

    const urlArg = matchWord(q, kw.url);
    if (urlArg) {
      const url = tryUrl(urlArg);
      setResults(url ? [url] : []);
      return;
    }

    const web = tryWebSearch(q, { google: kw.google, youtube: kw.youtube });
    if (web) {
      setResults([web]);
      return;
    }

    const sysArg = matchWord(q, kw.system);
    if (sysArg) {
      setResults(matchSystemCommands(sysArg));
      return;
    }

    // Task Manager: "<process> <lọc>"
    const psArg = matchWord(q, kw.process);
    if (psArg !== null) {
      const t = setTimeout(async () => {
        try {
          const list = await invoke<ProcInfo[]>("list_processes", { query: psArg });
          fresh(() => setResults(list.map((p) => ({
            id: `proc:${p.pid}`,
            title: p.name,
            subtitle: `PID ${p.pid} · ${p.mem_mb.toFixed(1)} MB — → để Kill`,
            kind: "process" as const,
            pid: p.pid,
            path: p.exe || undefined,
            icon: p.icon ?? undefined,
          }))));
        } catch { fresh(() => setResults([])); }
      }, 120);
      return () => clearTimeout(t);
    }

    // Trình quản lý mật khẩu trình duyệt: "pw <lọc>" (chỉ khi bật trong Settings)
    if (kw.password) {
      const pwArg = matchWord(q, kw.password);
      if (pwArg !== null) {
        const t = setTimeout(async () => {
          try {
            const list = await invoke<BrowserPassword[]>("list_passwords", { query: pwArg });
            fresh(() => setResults(list.length ? list.map((p, i) => ({
              id: `pw:${i}:${p.url}:${p.username}`,
              title: p.username || "(không có username)",
              subtitle: `${p.browser} · ${p.url} — Enter copy mật khẩu`,
              kind: "password" as const,
              text: p.password,
              secret: p.password,
              url: p.url,
            })) : [{
              id: "pw:empty", title: "Không tìm thấy mật khẩu khớp",
              subtitle: "Thử từ khóa khác (theo domain hoặc username)", kind: "password", text: "",
            }]));
          } catch (err) {
            fresh(() => setResults([{
              id: "pw:err", title: "Chưa bật tính năng đọc mật khẩu",
              subtitle: String(err), kind: "password", text: "",
            }]));
          }
        }, 200);
        return () => clearTimeout(t);
      }
    }

    // Mặc định: tìm app / file / folder
    setResults((prev) => prev.filter((p) => ["app", "file", "folder"].includes(p.kind)));
    const t = setTimeout(async () => {
      try {
        const resp = await invoke<BackendSearchResponse>("search_all", { query: q });
        if (seq.current !== mySeq) return;
        setEngine(resp.engine);
        const backend: ResultItemData[] = resp.results.map((r) => ({
          id: `${r.kind}:${r.path}`, title: r.title, subtitle: r.subtitle,
          kind: r.kind, path: r.path, icon: r.icon ?? undefined,
        }));
        setResults(backend);
      } catch { fresh(() => setResults([])); }
    }, 90);
    return () => clearTimeout(t);
  }, [query, snippets, workflows, hasCapToken, kw]);

  return { results, engine };
}

/** Phần phía sau keyword (giữ nguyên khoảng trắng nội bộ) */
function afterKeyword(q: string, keyword: string): string {
  const t = q.trim();
  if (t.toLowerCase() === keyword.toLowerCase()) return "";
  return t.slice(keyword.length).trim();
}
