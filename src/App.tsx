import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, useAnimationControls } from "framer-motion";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { SearchBar } from "./components/SearchBar";
import { ResultList } from "./components/ResultList";
import { ClipboardView } from "./components/ClipboardView";
import { KnowledgePreview } from "./components/KnowledgePreview";
import { actionsFor, ContextMenu, type CtxAction } from "./components/ContextMenu";
import { useKeyboardNav } from "./hooks/useKeyboardNav";
import { useSearch } from "./hooks/useSearch";
import { resolveKeywords, DEFAULT_KEYWORDS, type KwMap } from "./keywords";
import { applyTheme } from "./theme";
import type { AppSettings, ClipItem, ResultItemData, UiMode } from "./types";

export default function App() {
  const [mode, setMode] = useState<UiMode>("search");
  const [query, setQuery] = useState("");
  const [refreshKey, setRefreshKey] = useState(0);
  const [clipItems, setClipItems] = useState<ClipItem[]>([]);
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const controls = useAnimationControls();
  const [showTick, setShowTick] = useState(0);
  const lastSize = useRef({ w: 0, h: 0 });
  const [ctxOpen, setCtxOpen] = useState(false);
  const [ctxIndex, setCtxIndex] = useState(0);
  const [snipOpen, setSnipOpen] = useState(false);
  const [ocrItem, setOcrItem] = useState<ResultItemData>();
  const [expandedProcessGroups, setExpandedProcessGroups] = useState<Set<string>>(new Set());
  const [kw, setKw] = useState<KwMap>(DEFAULT_KEYWORDS);
  const [autoPaste, setAutoPaste] = useState(true);
  const [pwToHistory, setPwToHistory] = useState(false);

  const { results, engine } = useSearch(mode === "search" ? query : "", refreshKey, kw);
  const routedResults = results.flatMap((item): ResultItemData[] => {
    if (item.kind !== "process-group") return [item];
    const expanded = expandedProcessGroups.has(item.id);
    const group = { ...item, expanded };
    if (!expanded) return [group];
    const children = (item.processes || []).map((proc) => ({
      id: `proc:${proc.pid}`,
      title: proc.name,
      subtitle: `PID ${proc.pid} · ${proc.mem_mb.toFixed(1)} MB — → để Kill`,
      kind: "process" as const,
      pid: proc.pid,
      path: proc.exe || undefined,
      icon: proc.icon ?? undefined,
      isChild: true,
    }));
    return [group, ...children];
  });
  const visibleResults = mode === "search" && ocrItem ? [ocrItem] : routedResults;

  const navItems: unknown[] = mode === "clipboard" ? clipItems : mode === "search" ? visibleResults : [];
  const { index, setIndex, move } = useKeyboardNav(navItems);
  const selectedClip = mode === "clipboard" ? clipItems[index] : undefined;
  const selectedResult = mode === "search" ? visibleResults[index] : undefined;
  const showKnowledge = selectedResult?.kind === "knowledge";
  const detailKeywords = [kw.translate, kw.wiki, kw.formula, kw.review, kw.ocr].filter(Boolean);
  const detailTrigger = mode === "search" && detailKeywords.some((key) => {
    const lower = query.trim().toLowerCase();
    const trigger = key.toLowerCase();
    return lower === trigger || lower.startsWith(`${trigger} `);
  });
  const dirty = !!selectedClip && draft !== selectedClip.content;

  const toggleProcessGroup = (id: string, expand?: boolean) => {
    setExpandedProcessGroups((current) => {
      const next = new Set(current);
      const shouldExpand = expand ?? !next.has(id);
      if (shouldExpand) next.add(id); else next.delete(id);
      return next;
    });
  };

  // Nạp cấu hình (keyword, theme, auto-paste) khi khởi động + khi Settings lưu
  useEffect(() => {
    const loadSettings = () => {
      invoke<AppSettings>("get_settings")
        .then((s) => {
          setKw(resolveKeywords(s.keywords));
          setAutoPaste(s.auto_paste);
          setPwToHistory(s.password_to_history);
          applyTheme(s.theme);
        })
        .catch(() => {});
    };
    loadSettings();
    const unlisten = listen("winspot://settings-changed", loadSettings);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Nạp nội dung item được chọn vào khung edit
  useEffect(() => {
    setDraft(selectedClip?.content ?? "");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedClip?.id]);

  // Đóng context menu khi query / selection / mode thay đổi
  useEffect(() => {
    setCtxOpen(false);
    setCtxIndex(0);
    setSnipOpen(false);
  }, [query, index, mode]);

  // Watcher báo clipboard có item mới -> update list real-time nếu đang mở
  useEffect(() => {
    const unlisten = listen("clipboard://changed", () => {
      if (mode === "clipboard") setRefreshKey((k) => k + 1);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [mode]);

  // AUTO-SAVE: sửa nội dung trong preview là tự lưu sau 600ms (plan.md)
  useEffect(() => {
    if (!selectedClip) return;
    if (selectedClip.kind !== "text" && selectedClip.kind !== "link") return;
    if (draft === selectedClip.content) return;
    const t = setTimeout(() => {
      void saveDraft();
    }, 600);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft]);

  // Animation bung mở mượt mỗi lần cửa sổ hiện (không remount cả cây UI)
  useEffect(() => {
    controls.set({ opacity: 0, scale: 0.98, y: -6 });
    void controls.start({
      opacity: 1,
      scale: 1,
      y: 0,
      transition: { duration: 0.18, ease: [0.16, 1, 0.3, 1] },
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showTick]);

  // Backend báo chuẩn bị hiện cửa sổ (Alt+Space / Win+V / tray).
  // Thứ tự chống giật: reset UI về opacity 0 TRƯỚC -> show cửa sổ -> chạy animation.
  useEffect(() => {
    type Prepare = { mode?: string; prefill?: string };
    const unlisten = listen<Prepare | string>("winspot://prepare", (e) => {
      const payload = typeof e.payload === "string" ? { mode: e.payload } : e.payload;
      const nextMode = payload.mode === "clipboard" ? "clipboard" : "search";
      const prefill = payload.prefill ?? "";
      setMode(nextMode);
      setQuery(prefill);
      setOcrItem(undefined);
      setIndex(0);
      setRefreshKey((k) => k + 1);
      controls.set({ opacity: 0, scale: 0.98, y: -6 });
      requestAnimationFrame(async () => {
        const win = getCurrentWindow();
        await win.show().catch(() => {});
        await win.setFocus().catch(() => {});
        inputRef.current?.focus();
        // prefill có sẵn -> đặt con trỏ cuối để gõ tiếp; không có -> select toàn bộ
        if (prefill) {
          const el = inputRef.current;
          if (el) el.setSelectionRange(prefill.length, prefill.length);
        } else {
          inputRef.current?.select();
        }
        setShowTick((t) => t + 1); // kích hoạt animation bung mở
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setIndex]);

  // Khi ẩn: reset về trạng thái gọn nhất để lần mở sau không bị "nhảy" layout
  useEffect(() => {
    const unlisten = listen("winspot://hidden", () => {
      setMode("search");
      setQuery("");
      setOcrItem(undefined);
      setIndex(0);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [setIndex]);

  // Nạp clipboard history (lọc theo query) khi ở mode clipboard
  useEffect(() => {
    if (mode !== "clipboard") return;
    const t = setTimeout(() => {
      invoke<ClipItem[]>("get_clipboard_history", {
        limit: 50,
        filter: query.trim() || null,
      })
        .then(setClipItems)
        .catch(() => setClipItems([]));
    }, 60);
    return () => clearTimeout(t);
  }, [mode, query, refreshKey]);

  // Cửa sổ co giãn: search mode giãn theo kết quả, clipboard mode rộng cố định.
  // Chỉ gọi xuống Rust khi kích thước THỰC SỰ đổi để tránh giật khi gõ phím.
  useLayoutEffect(() => {
    const apply = (w: number, h: number) => {
      if (lastSize.current.w === w && lastSize.current.h === h) return;
      lastSize.current = { w, h };
      invoke("resize_window", { height: h, width: w }).catch(() => {});
    };
    if (mode === "clipboard") {
      apply(900, 520);
      return;
    }
    if (showKnowledge || detailTrigger) {
      apply(900, 520);
      return;
    }
    const el = panelRef.current;
    if (!el) return;
    apply(900, Math.max(Math.min(Math.round(el.scrollHeight), 640), 72));
  }, [visibleResults.length, mode, showKnowledge, detailTrigger]);

  const hide = () => {
    setQuery("");
    invoke("hide_and_trim").catch(() => {});
  };

  async function execute(item: ResultItemData) {
    try {
      switch (item.kind) {
        case "app":
        case "file":
        case "folder":
        case "fulltext":
          if (!item.path) return;
          hide();
          await invoke("open_path", { path: item.path });
          break;
        case "calc":
        case "snippet":
          if (item.text) await invoke("copy_text", { text: item.text });
          hide();
          break;
        case "web":
        case "capacities":
          if (!item.url) return;
          hide();
          await invoke("open_url", { url: item.url });
          break;
        case "cap-token":
          if (item.text) {
            await invoke("set_capacities_token", { token: item.text });
            setRefreshKey((k) => k + 1);
            setQuery(`${kw.fulltext} `);
          } else {
            setQuery("capacities token ");
          }
          break;
        case "window":
          if (item.hwnd == null) return;
          hide();
          await invoke("focus_window", { hwnd: item.hwnd });
          break;
        case "vscode":
          if (!item.path) return;
          hide();
          await invoke("open_vscode", { path: item.path });
          break;
        case "registry":
          if (!item.path) return;
          hide();
          await invoke("open_regedit", { path: item.path });
          break;
        case "service":
        case "process":
          // Enter -> mở context menu (Start/Stop/Restart hoặc Kill)
          setCtxOpen(true);
          setCtxIndex(0);
          break;
        case "process-group":
          toggleProcessGroup(item.id);
          break;
        case "password":
          if (!item.secret) return;
          if (pwToHistory) {
            await invoke("copy_text", { text: item.secret });
          } else {
            await invoke("copy_secret", { text: item.secret });
          }
          hide();
          break;
        case "generated":
        case "unit":
        case "time":
          if (item.text) await invoke("copy_text", { text: item.text });
          hide();
          break;
        case "knowledge":
          // OCR: Enter = copy văn bản (đã sửa); còn lại = dán như cũ
          if (item.action === "ocr") {
            if (item.text) await invoke("copy_text", { text: item.text });
            hide();
          } else if (item.text) {
            await invoke("paste_text", { text: item.text });
          }
          break;
        case "url":
          if (!item.url) return;
          hide();
          await invoke("open_url", { url: item.url });
          break;
        case "workflow":
          if (!item.text) return;
          hide();
          if (/^(https?|capacities|mailto):/i.test(item.text)) {
            await invoke("open_url", { url: item.text });
          } else if (/^[a-zA-Z]:\\/.test(item.text)) {
            await invoke("open_path", { path: item.text });
          } else {
            await invoke("copy_text", { text: item.text });
          }
          break;
        case "wf-cmd":
          if (item.action === "install" && item.text) {
            await invoke("install_workflow", { path: item.text });
          } else {
            await invoke("rescan_workflows");
          }
          setRefreshKey((k) => k + 1);
          setQuery("workflow");
          break;
        case "system":
          hide();
          await invoke("system_command", { action: item.action });
          break;
        case "terminal":
          hide();
          await invoke("run_in_terminal", { command: item.text });
          break;
        case "snippet-add":
          await invoke("add_snippet", {
            keyword: item.keyword,
            content: item.text,
          });
          setQuery(kw.snippet);
          setRefreshKey((k) => k + 1);
          break;
        case "settings":
          hide();
          await invoke("open_settings_window");
          break;
        case "ocr-cmd": {
          let text = "";
          let preview = "";
          try {
            text = await invoke<string>("capture_ocr");
            preview = text;
          } catch (err) {
            preview = `OCR không thành công:\n${String(err)}`;
          }
          setQuery("");
          setOcrItem({
            id: "ocr:result",
            title: text ? "Kết quả OCR" : "Không nhận dạng được",
            subtitle: text ? `${text.length} ký tự · Enter để dán` : "Thử chụp lại vùng rõ hơn",
            kind: "knowledge",
            action: "ocr",
            text,
            preview,
          });
          setIndex(0);
          const win = getCurrentWindow();
          await win.show().catch(() => {});
          await win.setFocus().catch(() => {});
          inputRef.current?.focus();
          break;
        }
      }
    } catch (err) {
      console.error("execute lỗi:", err);
    }
  }

  /** Lưu nội dung đã sửa vào SQLite (không đổi selection) */
  async function saveDraft() {
    if (!selectedClip || draft === selectedClip.content) return;
    const id = selectedClip.id;
    await invoke("update_clipboard_item", { id, content: draft }).catch(() => {});
    setClipItems((list) =>
      list.map((c) => (c.id === id ? { ...c, content: draft } : c))
    );
  }

  /** Enter trên clipboard: auto-paste vào cửa sổ trước, hoặc chỉ copy nếu tắt auto-paste */
  async function pasteClip(plain: boolean, target?: ClipItem) {
    const item = target ?? selectedClip;
    if (!item) return;
    if (item.id === selectedClip?.id) await saveDraft();
    setQuery("");
    if (autoPaste) {
      await invoke("paste_clipboard_item", { id: item.id, plain }).catch(() => {});
    } else {
      await invoke("copy_clipboard_item", { id: item.id }).catch(() => {});
      hide();
    }
  }

  async function togglePinSelected(target: ClipItem | undefined = selectedClip) {
    if (!target) return;
    const id = target.id;
    const pinned = await invoke<boolean>("toggle_pin", { id }).catch(() => null);
    if (pinned === null) return;
    setClipItems((list) => {
      const reordered = list
        .map((c) => (c.id === id ? { ...c, pinned } : c))
        .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.id - a.id);
      const nextIndex = reordered.findIndex((c) => c.id === id);
      requestAnimationFrame(() => setIndex(Math.max(0, nextIndex)));
      return reordered;
    });
    // Click nút ghim không được làm mất keyboard navigation của ô search.
    requestAnimationFrame(() => inputRef.current?.focus());
  }

  async function saveAsSnippet(keyword: string) {
    if (!selectedClip) return;
    await saveDraft();
    await invoke("add_snippet", {
      keyword,
      content: draft || selectedClip.content,
    }).catch(() => {});
    setSnipOpen(false);
    inputRef.current?.focus();
  }

  /** Thực thi một action trong Context Menu (plan2) */
  async function runCtxAction(item: ResultItemData, action: CtxAction) {
    setCtxOpen(false);
    try {
      switch (action.id) {
        case "open":
          await execute(item);
          break;
        case "run-admin":
          if (!item.path) return;
          hide();
          await invoke("run_as_admin", { path: item.path });
          break;
        case "open-location":
          if (!item.path) return;
          hide();
          await invoke("open_file_location", { path: item.path });
          break;
        case "copy-path":
          if (item.path) await invoke("copy_text", { text: item.path });
          hide();
          break;
        case "run-terminal":
          if (!item.path) return;
          hide();
          await invoke("run_in_terminal", { command: `"${item.path}"` });
          break;
        case "uninstall":
          if (!item.path) return;
          hide();
          await invoke("uninstall_app", { title: item.title, path: item.path });
          break;
        case "open-terminal-here":
          if (!item.path) return;
          hide();
          await invoke("run_in_terminal", { command: `cd /d "${item.path}"` });
          break;
        case "svc-start":
        case "svc-stop":
        case "svc-restart":
          if (!item.text) return;
          hide();
          await invoke("service_action", {
            name: item.text,
            action: action.id.replace("svc-", ""),
          });
          break;
        case "copy-text":
          if (item.text) await invoke("copy_text", { text: item.text });
          hide();
          break;
        case "paste-text":
          if (item.text) await invoke("paste_text", { text: item.text });
          break;
        case "open-source":
          if (!item.url) return;
          hide();
          await invoke("open_url", { url: item.url });
          break;
        case "kill":
        case "kill-tree":
          if (item.pid == null) return;
          hide();
          await invoke("kill_process", { pid: item.pid, tree: action.id === "kill-tree" });
          break;
        case "copy-pid":
          if (item.pid != null) await invoke("copy_text", { text: String(item.pid) });
          hide();
          break;
        case "copy-secret":
          if (item.secret) {
            if (pwToHistory) await invoke("copy_text", { text: item.secret });
            else await invoke("copy_secret", { text: item.secret });
          }
          hide();
          break;
        case "copy-username":
          if (item.title) await invoke("copy_text", { text: item.title });
          hide();
          break;
      }
    } catch (err) {
      console.error("context action lỗi:", err);
    }
  }

  async function deleteSelectedClip(target: ClipItem | undefined = selectedClip) {
    if (!target) return;
    const id = target.id;
    const selectedId = selectedClip?.id;
    await invoke("delete_clipboard_item", { id }).catch(() => {});
    setClipItems((list) => {
      const filtered = list.filter((c) => c.id !== id);
      const nextIndex = selectedId === id
        ? Math.min(index, Math.max(0, filtered.length - 1))
        : Math.max(0, filtered.findIndex((c) => c.id === selectedId));
      requestAnimationFrame(() => setIndex(nextIndex));
      return filtered;
    });
    requestAnimationFrame(() => inputRef.current?.focus());
  }

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    // Context menu đang mở: điều hướng bên trong menu
    if (ctxOpen && mode === "search") {
      const item = visibleResults[index];
      const actions = item ? actionsFor(item) : [];
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setCtxIndex((i) => (actions.length ? (i + 1) % actions.length : 0));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setCtxIndex((i) => (actions.length ? (i - 1 + actions.length) % actions.length : 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        if (item && actions[ctxIndex]) void runCtxAction(item, actions[ctxIndex]);
        return;
      }
      if (e.key === "ArrowLeft" || e.key === "Escape") {
        e.preventDefault();
        setCtxOpen(false);
        return;
      }
      // Phím khác: đóng menu rồi xử lý bình thường
      setCtxOpen(false);
    }

    const currentItem = mode === "search" ? visibleResults[index] : undefined;
    if (currentItem?.kind === "process-group" && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
      e.preventDefault();
      toggleProcessGroup(currentItem.id, e.key === "ArrowRight");
      return;
    }

    // "→" ở cuối ô nhập: mở Context Menu cho kết quả đang chọn
    if (
      e.key === "ArrowRight" &&
      mode === "search" &&
      !ctxOpen &&
      (e.target as HTMLInputElement).selectionStart === query.length
    ) {
      const item = visibleResults[index];
      if (item && actionsFor(item).length > 0) {
        e.preventDefault();
        setCtxOpen(true);
        setCtxIndex(0);
        return;
      }
    }

    if (e.key === "ArrowDown") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      // Enter = dán nguyên bản, Shift+Enter = dán plain text (plan.md)
      if (mode === "clipboard") void pasteClip(e.shiftKey);
      else if (visibleResults[index]) void execute(visibleResults[index]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (snipOpen) setSnipOpen(false);
      else if (query) setQuery("");
      else hide();
    } else if (mode === "clipboard" && e.key === "Delete" && !query) {
      e.preventDefault();
      void deleteSelectedClip();
    } else if (mode === "clipboard" && e.ctrlKey && e.key.toLowerCase() === "p") {
      e.preventDefault();
      void togglePinSelected();
    } else if (mode === "clipboard" && e.ctrlKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (selectedClip && (selectedClip.kind === "text" || selectedClip.kind === "link")) {
        setSnipOpen(true);
      }
    } else if (mode === "clipboard" && e.ctrlKey && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      const it = clipItems[parseInt(e.key, 10) - 1];
      if (it) void pasteClip(false, it);
    } else if (e.key === "Tab") {
      e.preventDefault();
      if (mode === "clipboard") {
        textareaRef.current?.focus();
      } else {
        const it = visibleResults[index];
        if (it && ["app", "file", "folder"].includes(it.kind)) setQuery(it.title);
      }
    } else if (mode === "clipboard" && e.ctrlKey && e.key.toLowerCase() === "e") {
      e.preventDefault();
      textareaRef.current?.focus();
    } else if (mode === "clipboard" && e.ctrlKey && e.key.toLowerCase() === "d") {
      e.preventDefault();
      void deleteSelectedClip();
    } else if (mode === "clipboard" && e.ctrlKey && e.key.toLowerCase() === "l") {
      e.preventDefault();
      invoke("clear_clipboard_history")
        .then(() => setRefreshKey((k) => k + 1))
        .catch(() => {});
    } else if (mode === "search" && e.ctrlKey && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      const it = visibleResults[parseInt(e.key, 10) - 1];
      if (it) void execute(it);
    }
  };

  return (
    <motion.div
      ref={panelRef}
      initial={false}
      animate={controls}
      className={`relative flex flex-col rounded-2xl overflow-hidden
                 bg-white/95 dark:bg-zinc-900/95
                 border border-black/10 dark:border-white/10
                 ${mode !== "search" ? "h-screen" : ""}`}
    >
      <SearchBar
        ref={inputRef}
        value={query}
        mode={mode}
        onChange={(value) => {
          setOcrItem(undefined);
          setIndex(0);
          setQuery(value);
        }}
        onKeyDown={onKeyDown}
      />

      {mode === "search" ? (
        <>
          <div className={showKnowledge ? "flex h-[414px]" : ""}>
            <div className={showKnowledge ? "w-[360px] shrink-0 overflow-hidden" : ""}>
              <ResultList
                items={visibleResults}
                selectedIndex={index}
                onExecute={(it) => void execute(it)}
                onHover={setIndex}
              />
            </div>
            {showKnowledge && selectedResult && (
              <KnowledgePreview
                item={selectedResult}
                editable={selectedResult.action === "ocr"}
                onEdit={(text) =>
                  setOcrItem((prev) => (prev ? { ...prev, text, preview: text } : prev))
                }
                onExit={() => inputRef.current?.focus()}
                onPaste={() => void invoke("paste_text", { text: selectedResult.text })}
                onCopy={() => {
                  void invoke("copy_text", { text: selectedResult.text ?? "" });
                  hide();
                }}
                onOpen={() => {
                  if (selectedResult.url) void invoke("open_url", { url: selectedResult.url });
                }}
                onSave={() => void invoke("save_study_word", {
                  word: selectedResult.keyword,
                  translation: selectedResult.text,
                  details: selectedResult.preview || "",
                })}
                onTranslate={() => {
                  const text = selectedResult.text || "";
                  setOcrItem(undefined);
                  setQuery(text ? `${kw.translate} ${text}` : `${kw.translate} `);
                }}
              />
            )}
          </div>
          {ctxOpen && visibleResults[index] && (
            <ContextMenu
              item={visibleResults[index]}
              actions={actionsFor(visibleResults[index])}
              selectedIndex={ctxIndex}
              onRun={(a) => void runCtxAction(visibleResults[index], a)}
              onHover={setCtxIndex}
            />
          )}
        </>
      ) : (
        <ClipboardView
          items={clipItems}
          selectedIndex={index}
          draft={draft}
          dirty={dirty}
          onDraftChange={setDraft}
          onSelect={setIndex}
          onPaste={(plain) => void pasteClip(plain)}
          onTogglePin={() => void togglePinSelected()}
          onTogglePinItem={(item) => void togglePinSelected(item)}
          onDeleteItem={(item) => void deleteSelectedClip(item)}
          onFocusInput={() => inputRef.current?.focus()}
          textareaRef={textareaRef}
          snipOpen={snipOpen}
          onSnipSubmit={(kw) => void saveAsSnippet(kw)}
          onSnipClose={() => {
            setSnipOpen(false);
            inputRef.current?.focus();
          }}
        />
      )}

      {(mode === "clipboard" || visibleResults.length > 0) && (
        <div
          className="flex items-center justify-between px-4 py-1.5 shrink-0
                     border-t border-black/5 dark:border-white/10
                     text-[10.5px] text-zinc-400 dark:text-zinc-500"
        >
          <span>
            HeaSpot
            {mode === "search" && (
              <span className="ml-2 opacity-70">
                engine: {engine === "everything" ? "Everything ⚡" : "Index nội bộ"}
              </span>
            )}
          </span>
          <span>
            {mode === "clipboard"
              ? "Enter dán · ⇧Enter dán thuần · ⌃1-9 dán nhanh · ⌃P ghim · ⌃S snippet · Del xoá"
              : "↑↓ chọn · Enter mở · → menu · Tab điền · Esc đóng"}
          </span>
        </div>
      )}
    </motion.div>
  );
}
