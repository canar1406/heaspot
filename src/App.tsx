import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, useAnimationControls } from "framer-motion";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { SearchBar } from "./components/SearchBar";
import { ResultList } from "./components/ResultList";
import { ClipboardView } from "./components/ClipboardView";
import { actionsFor, ContextMenu, type CtxAction } from "./components/ContextMenu";
import { useKeyboardNav } from "./hooks/useKeyboardNav";
import { useSearch } from "./hooks/useSearch";
import type { ClipItem, ResultItemData, UiMode } from "./types";

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

  const { results, engine } = useSearch(mode === "search" ? query : "", refreshKey);

  const navItems: unknown[] = mode === "clipboard" ? clipItems : results;
  const { index, setIndex, move } = useKeyboardNav(navItems);
  const selectedClip = mode === "clipboard" ? clipItems[index] : undefined;
  const dirty = !!selectedClip && draft !== selectedClip.content;

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
    const unlisten = listen<string>("winspot://prepare", (e) => {
      setMode(e.payload === "clipboard" ? "clipboard" : "search");
      setQuery("");
      setIndex(0);
      setRefreshKey((k) => k + 1);
      controls.set({ opacity: 0, scale: 0.98, y: -6 });
      requestAnimationFrame(async () => {
        const win = getCurrentWindow();
        await win.show().catch(() => {});
        await win.setFocus().catch(() => {});
        inputRef.current?.focus();
        inputRef.current?.select();
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
    const el = panelRef.current;
    if (!el) return;
    apply(680, Math.max(Math.min(Math.round(el.scrollHeight), 640), 72));
  }, [results.length, mode]);

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
            setQuery("in ");
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
          // Enter trên service -> mở context menu Start/Stop/Restart
          setCtxOpen(true);
          setCtxIndex(0);
          break;
        case "generated":
        case "unit":
        case "time":
          if (item.text) await invoke("copy_text", { text: item.text });
          hide();
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
          setQuery(";");
          setRefreshKey((k) => k + 1);
          break;
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

  /** AUTO-PASTE: dán item (bản đã sửa nếu có) thẳng vào cửa sổ trước đó */
  async function pasteClip(plain: boolean, target?: ClipItem) {
    const item = target ?? selectedClip;
    if (!item) return;
    if (item.id === selectedClip?.id) await saveDraft();
    setQuery("");
    await invoke("paste_clipboard_item", { id: item.id, plain }).catch(() => {});
  }

  async function togglePinSelected() {
    if (!selectedClip) return;
    const id = selectedClip.id;
    const pinned = await invoke<boolean>("toggle_pin", { id }).catch(() => null);
    if (pinned === null) return;
    setClipItems((list) => list.map((c) => (c.id === id ? { ...c, pinned } : c)));
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
      }
    } catch (err) {
      console.error("context action lỗi:", err);
    }
  }

  async function deleteSelectedClip() {
    if (!selectedClip) return;
    const id = selectedClip.id;
    await invoke("delete_clipboard_item", { id }).catch(() => {});
    setClipItems((list) => list.filter((c) => c.id !== id));
  }

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    // Context menu đang mở: điều hướng bên trong menu
    if (ctxOpen && mode === "search") {
      const item = results[index];
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

    // "→" ở cuối ô nhập: mở Context Menu cho kết quả đang chọn
    if (
      e.key === "ArrowRight" &&
      mode === "search" &&
      !ctxOpen &&
      (e.target as HTMLInputElement).selectionStart === query.length
    ) {
      const item = results[index];
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
      else if (results[index]) void execute(results[index]);
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
        const it = results[index];
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
        .then(() => setClipItems([]))
        .catch(() => {});
    } else if (mode === "search" && e.ctrlKey && /^[1-9]$/.test(e.key)) {
      e.preventDefault();
      const it = results[parseInt(e.key, 10) - 1];
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
                 ${mode === "clipboard" ? "h-screen" : ""}`}
    >
      <SearchBar
        ref={inputRef}
        value={query}
        mode={mode}
        onChange={setQuery}
        onKeyDown={onKeyDown}
      />

      {mode === "search" ? (
        <>
          <ResultList
            items={results}
            selectedIndex={index}
            onExecute={(it) => void execute(it)}
            onHover={setIndex}
          />
          {ctxOpen && results[index] && (
            <ContextMenu
              item={results[index]}
              actions={actionsFor(results[index])}
              selectedIndex={ctxIndex}
              onRun={(a) => void runCtxAction(results[index], a)}
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

      {(mode === "clipboard" || results.length > 0) && (
        <div
          className="flex items-center justify-between px-4 py-1.5 shrink-0
                     border-t border-black/5 dark:border-white/10
                     text-[10.5px] text-zinc-400 dark:text-zinc-500"
        >
          <span>
            WinSpot
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
