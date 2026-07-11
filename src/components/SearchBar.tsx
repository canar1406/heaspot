import { forwardRef } from "react";
import type { UiMode } from "../types";
import logo from "../assets/logo.png";

interface Props {
  value: string;
  mode: UiMode;
  onChange: (v: string) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
}

/** Ô nhập liệu chính — luôn giữ focus, keyboard-first */
export const SearchBar = forwardRef<HTMLInputElement, Props>(
  ({ value, mode, onChange, onKeyDown }, ref) => {
    return (
      <div className="flex items-center gap-3 px-5 h-[64px] shrink-0">
        <img
          src={logo}
          alt="heavietnam"
          draggable={false}
          className="w-7 h-7 shrink-0 object-contain select-none"
        />
        <input
          ref={ref}
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          autoFocus
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          placeholder={
            mode === "clipboard"
              ? "Lọc clipboard history…"
              : "Tìm app, file… (g web · in nội-dung · = tính · conv đổi · tr dịch)"
          }
          className="flex-1 bg-transparent outline-none border-none text-[20px]
                     text-zinc-900 dark:text-zinc-50
                     placeholder-zinc-400 dark:placeholder-zinc-500"
        />
        {mode === "clipboard" && (
          <span className="text-[10px] font-medium uppercase tracking-wider text-violet-500 dark:text-violet-400 border border-violet-400/40 rounded px-1.5 py-0.5">
            Clipboard
          </span>
        )}
      </div>
    );
  }
);

SearchBar.displayName = "SearchBar";
