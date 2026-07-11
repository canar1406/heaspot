import { invoke } from "@tauri-apps/api/core";
import { RefObject, useEffect, useRef, useState } from "react";
import type { ClipItem } from "../types";
import { Icon } from "./Icon";

interface Props {
  items: ClipItem[];
  selectedIndex: number;
  draft: string;
  dirty: boolean;
  onDraftChange: (v: string) => void;
  onSelect: (i: number) => void;
  /** Dán item đang chọn vào cửa sổ trước đó (plain = văn bản thuần) */
  onPaste: (plain: boolean) => void;
  onTogglePin: () => void;
  onFocusInput: () => void;
  textareaRef: RefObject<HTMLTextAreaElement>;
  /** Form "Lưu thành Snippet" (Ctrl+S) */
  snipOpen: boolean;
  onSnipSubmit: (keyword: string) => void;
  onSnipClose: () => void;
}

function firstLine(s: string): string {
  const line = s.trimStart().split("\n")[0];
  return line.length > 56 ? line.slice(0, 56) + "…" : line || "(trống)";
}

function kindIcon(kind: ClipItem["kind"]) {
  if (kind === "link") return "web" as const;
  if (kind === "files") return "folder" as const;
  return "clipboard" as const;
}

/** WinSpot Clipboard: list Alfred-style + preview edit + pin + ảnh */
export function ClipboardView({
  items,
  selectedIndex,
  draft,
  dirty,
  onDraftChange,
  onSelect,
  onPaste,
  onTogglePin,
  onFocusInput,
  textareaRef,
  snipOpen,
  onSnipSubmit,
  onSnipClose,
}: Props) {
  const listRef = useRef<HTMLDivElement>(null);
  const snipInputRef = useRef<HTMLInputElement>(null);
  const [fullImage, setFullImage] = useState<string>("");

  const sel = items[selectedIndex];

  useEffect(() => {
    const el = listRef.current?.children[selectedIndex] as HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  // Nạp ảnh full-size cho preview khi chọn item ảnh
  useEffect(() => {
    setFullImage("");
    if (sel?.kind === "image") {
      invoke<string>("get_clip_image", { id: sel.id })
        .then(setFullImage)
        .catch(() => setFullImage(sel.thumb));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel?.id]);

  useEffect(() => {
    if (snipOpen) requestAnimationFrame(() => snipInputRef.current?.focus());
  }, [snipOpen]);

  if (items.length === 0) {
    return (
      <div className="flex-1 min-h-0 flex items-center justify-center border-t border-black/5 dark:border-white/10 text-[13px] text-zinc-400 dark:text-zinc-500">
        Chưa có gì trong clipboard history — cứ copy như bình thường, HeaSpot sẽ ghi nhớ
      </div>
    );
  }

  const editable = sel && (sel.kind === "text" || sel.kind === "link");

  const onTextareaKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onFocusInput();
    } else if (e.key === "Enter" && !e.shiftKey) {
      // Nội dung đang sửa đã tự lưu -> chỉ cần Enter là dán luôn.
      // Shift+Enter để xuống dòng (soạn nhiều dòng).
      e.preventDefault();
      onPaste(false);
    } else if (e.ctrlKey && e.key === "Enter") {
      e.preventDefault();
      onPaste(true);
    }
  };

  return (
    <div className="relative flex flex-1 min-h-0 border-t border-black/5 dark:border-white/10">
      {/* Cột trái: danh sách */}
      <div
        ref={listRef}
        className="w-[300px] shrink-0 overflow-y-auto py-2 border-r border-black/5 dark:border-white/10"
      >
        {items.map((c, i) => (
          <div
            key={c.id}
            onClick={() => onSelect(i)}
            onDoubleClick={() => onPaste(false)}
            className={`flex items-center gap-2 mx-2 px-2.5 py-1.5 rounded-lg cursor-default transition-colors duration-75
              ${
                i === selectedIndex
                  ? "bg-blue-600 text-white"
                  : "text-zinc-800 dark:text-zinc-100 hover:bg-black/5 dark:hover:bg-white/5"
              }`}
          >
            {c.kind === "image" && c.thumb ? (
              <img
                src={c.thumb}
                alt=""
                className="w-9 h-7 shrink-0 object-cover rounded border border-black/10 dark:border-white/10"
              />
            ) : (
              <span
                className={`shrink-0 ${
                  i === selectedIndex ? "text-blue-100" : "text-zinc-400 dark:text-zinc-500"
                }`}
              >
                <Icon kind={kindIcon(c.kind)} size={14} />
              </span>
            )}
            <div className="flex-1 min-w-0">
              <div className="truncate text-[12.5px] font-medium leading-tight">
                {c.kind === "image" ? "Ảnh" : firstLine(c.content)}
              </div>
              <div
                className={`text-[10px] mt-0.5 truncate ${
                  i === selectedIndex ? "text-blue-100" : "text-zinc-400 dark:text-zinc-500"
                }`}
              >
                {c.source_app ? `${c.source_app} · ` : ""}
                {c.created_at}
              </div>
            </div>
            {c.pinned && (
              <span
                title="Đã ghim"
                className={`flex items-center justify-center w-5 h-5 rounded ${i === selectedIndex ? "text-amber-200 bg-white/10" : "text-amber-600 bg-amber-500/10"}`}
              >
                <svg viewBox="0 0 24 24" className="w-3 h-3 fill-current" aria-hidden="true">
                  <path d="M16 3a1 1 0 0 1 .8 1.6L15.2 7H17a1 1 0 0 1 .8 1.6L15 12.33V16l1.7 1.7a1 1 0 0 1-.7 1.7h-3v2.1a1 1 0 1 1-2 0v-2.1H8a1 1 0 0 1-.7-1.7L9 16v-3.67L6.2 8.6A1 1 0 0 1 7 7h1.8L7.2 4.6A1 1 0 0 1 8 3h8Z" />
                </svg>
              </span>
            )}
            {i < 9 && (
              <span
                className={`text-[10px] tabular-nums shrink-0 ${
                  i === selectedIndex ? "text-blue-100" : "text-zinc-400 dark:text-zinc-500"
                }`}
              >
                ⌃{i + 1}
              </span>
            )}
          </div>
        ))}
      </div>

      {/* Cột phải: preview */}
      <div className="flex-1 min-w-0 flex flex-col">
        <div className="flex items-center gap-2 px-4 pt-2.5 pb-1 text-[10px] uppercase tracking-wider text-zinc-400 dark:text-zinc-500 shrink-0">
          <span className="font-semibold">
            {sel?.kind === "image"
              ? "Ảnh"
              : sel?.kind === "files"
                ? "Files"
                : sel?.kind === "link"
                  ? "Link"
                  : "Text"}
          </span>
          <button
            type="button"
            onMouseDown={(e) => e.preventDefault()}
            onClick={onTogglePin}
            title="Ghim / bỏ ghim (Ctrl+P)"
            aria-pressed={!!sel?.pinned}
            className={`normal-case inline-flex items-center gap-1 px-2 py-1 rounded-md border transition-colors ${
              sel?.pinned
                ? "text-amber-700 dark:text-amber-300 bg-amber-500/15 border-amber-500/35"
                : "text-zinc-500 dark:text-zinc-300 bg-black/[.03] dark:bg-white/[.06] border-black/10 dark:border-white/10 hover:text-amber-600 hover:border-amber-500/30"
            }`}
          >
            <svg viewBox="0 0 24 24" className="w-3 h-3 fill-current" aria-hidden="true">
              <path d="M16 3a1 1 0 0 1 .8 1.6L15.2 7H17a1 1 0 0 1 .8 1.6L15 12.33V16l1.7 1.7a1 1 0 0 1-.7 1.7h-3v2.1a1 1 0 1 1-2 0v-2.1H8a1 1 0 0 1-.7-1.7L9 16v-3.67L6.2 8.6A1 1 0 0 1 7 7h1.8L7.2 4.6A1 1 0 0 1 8 3h8Z" />
            </svg>
            {sel?.pinned ? "Đã ghim" : "Ghim"}
          </button>
          <span>·</span>
          <span>{sel?.created_at}</span>
          {sel?.kind !== "image" && (
            <>
              <span>·</span>
              <span>{draft.length} ký tự</span>
            </>
          )}
          {dirty && (
            <span className="ml-auto normal-case font-medium text-emerald-500">
              ● đang tự lưu…
            </span>
          )}
        </div>

        {sel?.kind === "image" ? (
          <div className="flex-1 min-h-0 flex items-center justify-center p-3 overflow-hidden">
            <img
              src={fullImage || sel.thumb}
              alt=""
              className="max-w-full max-h-full object-contain rounded-lg border border-black/10 dark:border-white/10"
            />
          </div>
        ) : (
          <textarea
            ref={textareaRef}
            value={draft}
            onChange={(e) => editable && onDraftChange(e.target.value)}
            onKeyDown={onTextareaKeyDown}
            readOnly={!editable}
            spellCheck={false}
            className="flex-1 min-h-0 resize-none bg-transparent outline-none px-4 py-2
                       text-[13px] leading-relaxed select-text cursor-text
                       text-zinc-800 dark:text-zinc-100"
          />
        )}

        <div className="px-4 py-1.5 text-[10px] text-zinc-400 dark:text-zinc-500 border-t border-black/5 dark:border-white/10 shrink-0">
          Sửa là tự lưu · Enter dán (⇧Enter xuống dòng) · Esc quay lại ô tìm kiếm
        </div>
      </div>

      {/* Form Ctrl+S: biến item thành Snippet */}
      {snipOpen && sel && (
        <div className="absolute inset-0 z-30 flex items-center justify-center bg-black/30">
          <div className="w-[360px] rounded-xl bg-white dark:bg-zinc-800 border border-black/10 dark:border-white/10 shadow-2xl p-4">
            <div className="text-[13px] font-medium mb-1 text-zinc-800 dark:text-zinc-100">
              Lưu thành Snippet
            </div>
            <div className="text-[11px] text-zinc-500 dark:text-zinc-400 mb-3 truncate">
              {firstLine(draft || sel.content)}
            </div>
            <input
              ref={snipInputRef}
              type="text"
              placeholder="Nhập keyword gõ tắt (VD: mail)…"
              spellCheck={false}
              className="w-full px-3 py-2 rounded-lg text-[13px] outline-none
                         bg-black/5 dark:bg-white/10 text-zinc-900 dark:text-zinc-50
                         placeholder-zinc-400"
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") {
                  const kw = (e.target as HTMLInputElement).value.trim();
                  if (kw) onSnipSubmit(kw);
                } else if (e.key === "Escape") {
                  onSnipClose();
                }
              }}
            />
            <div className="mt-2 text-[10px] text-zinc-400 dark:text-zinc-500">
              Enter lưu · Esc huỷ — dùng lại bằng cách gõ <b>;keyword</b>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
