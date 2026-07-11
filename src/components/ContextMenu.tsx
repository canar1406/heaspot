import { Icon } from "./Icon";
import type { ResultItemData } from "../types";

export interface CtxAction {
  id: string;
  label: string;
}

/** Danh sách action ngữ cảnh theo loại kết quả (plan2 — Context Menu) */
export function actionsFor(item: ResultItemData): CtxAction[] {
  switch (item.kind) {
    case "app":
      return [
        { id: "open", label: "Mở" },
        { id: "run-admin", label: "Run as administrator" },
        { id: "open-location", label: "Open file location" },
        { id: "copy-path", label: "Copy path" },
        { id: "run-terminal", label: "Run in Terminal" },
      ];
    case "file":
    case "fulltext":
      return [
        { id: "open", label: "Mở" },
        { id: "open-location", label: "Open file location" },
        { id: "copy-path", label: "Copy path" },
      ];
    case "folder":
      return [
        { id: "open", label: "Mở" },
        { id: "copy-path", label: "Copy path" },
        { id: "open-terminal-here", label: "Mở Terminal tại đây" },
      ];
    case "vscode":
      return [
        { id: "open", label: "Mở bằng VS Code" },
        { id: "open-location", label: "Open file location" },
        { id: "copy-path", label: "Copy path" },
      ];
    case "service":
      return [
        { id: "svc-start", label: "Start service (admin)" },
        { id: "svc-stop", label: "Stop service (admin)" },
        { id: "svc-restart", label: "Restart service (admin)" },
        { id: "copy-text", label: "Copy tên service" },
      ];
    case "registry":
      return [
        { id: "open", label: "Mở trong Registry Editor" },
        { id: "copy-path", label: "Copy đường dẫn key" },
      ];
    default:
      return item.text
        ? [{ id: "copy-text", label: "Copy nội dung" }]
        : [];
  }
}

interface Props {
  item: ResultItemData;
  actions: CtxAction[];
  selectedIndex: number;
  onRun: (action: CtxAction) => void;
  onHover: (i: number) => void;
}

/** Bảng action trượt ra bên phải khi bấm mũi tên → (kiểu Windows Search) */
export function ContextMenu({ item, actions, selectedIndex, onRun, onHover }: Props) {
  return (
    <div
      className="absolute right-3 top-[64px] z-20 w-72 rounded-xl overflow-hidden
                 bg-white dark:bg-zinc-800
                 border border-black/10 dark:border-white/10
                 shadow-[0_12px_32px_rgba(0,0,0,0.35)]"
    >
      <div className="px-3 py-2 text-[11px] font-medium truncate border-b border-black/5 dark:border-white/10 text-zinc-500 dark:text-zinc-400">
        {item.title}
      </div>
      <div className="py-1.5">
        {actions.map((a, i) => (
          <div
            key={a.id}
            onClick={() => onRun(a)}
            onMouseMove={() => onHover(i)}
            className={`flex items-center gap-2 mx-1.5 px-2.5 py-1.5 rounded-lg text-[13px] cursor-default
              ${
                i === selectedIndex
                  ? "bg-blue-600 text-white"
                  : "text-zinc-800 dark:text-zinc-100 hover:bg-black/5 dark:hover:bg-white/5"
              }`}
          >
            <Icon kind="system" size={13} />
            <span className="truncate">{a.label}</span>
          </div>
        ))}
      </div>
      <div className="px-3 py-1.5 text-[10px] text-zinc-400 dark:text-zinc-500 border-t border-black/5 dark:border-white/10">
        ↑↓ chọn · Enter chạy · ← đóng
      </div>
    </div>
  );
}
