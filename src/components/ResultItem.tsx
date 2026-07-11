import type { ResultItemData } from "../types";
import { Icon } from "./Icon";

const ICON_BG: Partial<Record<ResultItemData["kind"], string>> = {
  app: "bg-blue-500/15 text-blue-600 dark:text-blue-400",
  file: "bg-zinc-500/15 text-zinc-600 dark:text-zinc-300",
  folder: "bg-amber-500/15 text-amber-600 dark:text-amber-400",
  calc: "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400",
  web: "bg-sky-500/15 text-sky-600 dark:text-sky-400",
  system: "bg-red-500/15 text-red-600 dark:text-red-400",
  terminal: "bg-zinc-800/15 text-zinc-700 dark:text-zinc-200",
  snippet: "bg-violet-500/15 text-violet-600 dark:text-violet-400",
  "snippet-add": "bg-violet-500/15 text-violet-600 dark:text-violet-400",
  fulltext: "bg-teal-500/15 text-teal-600 dark:text-teal-400",
  knowledge: "bg-indigo-500/15 text-indigo-600 dark:text-indigo-400",
  settings: "bg-zinc-500/15 text-zinc-600 dark:text-zinc-300",
  "ocr-cmd": "bg-violet-500/15 text-violet-600 dark:text-violet-400",
  capacities: "bg-indigo-500/15 text-indigo-600 dark:text-indigo-400",
  "cap-token": "bg-indigo-500/15 text-indigo-600 dark:text-indigo-400",
  clipboard: "bg-violet-500/15 text-violet-600 dark:text-violet-400",
  window: "bg-cyan-500/15 text-cyan-600 dark:text-cyan-400",
  vscode: "bg-blue-500/15 text-blue-600 dark:text-blue-400",
  service: "bg-orange-500/15 text-orange-600 dark:text-orange-400",
  registry: "bg-rose-500/15 text-rose-600 dark:text-rose-400",
  generated: "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400",
  unit: "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400",
  time: "bg-sky-500/15 text-sky-600 dark:text-sky-400",
  url: "bg-sky-500/15 text-sky-600 dark:text-sky-400",
  workflow: "bg-fuchsia-500/15 text-fuchsia-600 dark:text-fuchsia-400",
  "wf-cmd": "bg-fuchsia-500/15 text-fuchsia-600 dark:text-fuchsia-400",
  process: "bg-red-500/15 text-red-600 dark:text-red-400",
  "process-group": "bg-red-500/15 text-red-600 dark:text-red-400",
  password: "bg-amber-500/15 text-amber-600 dark:text-amber-400",
};

interface Props {
  item: ResultItemData;
  selected: boolean;
  shortcut?: string;
  onClick: () => void;
  onHover: () => void;
}

export function ResultItem({ item, selected, shortcut, onClick, onHover }: Props) {
  return (
    <div
      onClick={onClick}
      onMouseMove={onHover}
      className={`flex items-center gap-3 mx-2 px-3 py-2 rounded-lg cursor-default transition-colors duration-75
        ${item.isChild ? "ml-8 border-l border-zinc-300/50 dark:border-zinc-600/50" : ""}
        ${
          selected
            ? "bg-blue-600 text-white"
            : "text-zinc-800 dark:text-zinc-100 hover:bg-black/5 dark:hover:bg-white/5"
        }`}
    >
      {item.icon ? (
        <img
          src={item.icon}
          alt=""
          draggable={false}
          className="w-8 h-8 shrink-0 object-contain select-none"
        />
      ) : (
        <span
          className={`flex items-center justify-center w-8 h-8 rounded-md shrink-0
            ${selected ? "bg-white/20 text-white" : ICON_BG[item.kind] ?? "bg-zinc-500/15"}`}
        >
          <Icon kind={item.kind} />
        </span>
      )}
      <div className="flex-1 min-w-0">
        <div className="truncate text-[14px] font-medium leading-tight">
          {item.title}
        </div>
        {item.subtitle && (
          <div
            className={`truncate text-[11.5px] leading-tight mt-0.5
              ${selected ? "text-blue-100" : "text-zinc-500 dark:text-zinc-400"}`}
          >
            {item.subtitle}
          </div>
        )}
      </div>
      {shortcut && (
        <span
          className={`text-[10px] shrink-0 tabular-nums font-medium
            ${selected ? "text-blue-100" : "text-zinc-400 dark:text-zinc-500"}`}
        >
          {shortcut}
        </span>
      )}
      {item.kind === "process-group" && (
        <span className={`text-[12px] transition-transform ${item.expanded ? "rotate-90" : ""}`}>›</span>
      )}
    </div>
  );
}
