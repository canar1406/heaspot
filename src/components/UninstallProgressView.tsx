import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

export type UninstallProgress = {
  operation_id: string;
  app_name: string;
  stage: string;
  detail: string;
  percent: number | null;
  done: boolean;
  success: boolean;
};

export function UninstallProgressView() {
  const [progress, setProgress] = useState<UninstallProgress>();

  useEffect(() => {
    const sync = () => {
      void invoke<UninstallProgress | null>("get_uninstall_progress")
        .then((value) => value && setProgress(value))
        .catch(() => {});
    };
    sync();
    // Events provide immediate updates; polling is a recovery path for a
    // WebView that was minimized/suspended while a long vendor uninstaller ran.
    const poll = window.setInterval(sync, 750);
    const unlisten = listen<UninstallProgress>("uninstall://progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      window.clearInterval(poll);
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  const percent = progress?.percent;
  const color = progress?.done
    ? progress.success
      ? "bg-emerald-500"
      : "bg-red-500"
    : "bg-blue-500";

  return (
    <main className="flex h-full flex-col bg-zinc-50 px-5 py-4 text-zinc-900 dark:bg-zinc-900 dark:text-zinc-50">
      <div className="flex items-start gap-3">
        <div className={`mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl text-white ${color}`}>
          {progress?.done ? (progress.success ? "✓" : "!") : "↻"}
        </div>
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-[15px] font-semibold">
            {progress?.app_name || "Gỡ cài đặt ứng dụng"}
          </h1>
          <p className="mt-0.5 text-[12px] text-zinc-500 dark:text-zinc-400">
            {progress?.stage || "Đang chuẩn bị…"}
          </p>
        </div>
        {percent !== null && percent !== undefined && (
          <span className="text-[13px] font-semibold tabular-nums text-zinc-600 dark:text-zinc-300">
            {percent}%
          </span>
        )}
      </div>

      <div className="mt-4 h-2.5 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-700">
        {percent === null || percent === undefined ? (
          <div className="h-full w-1/3 animate-pulse rounded-full bg-blue-500" />
        ) : (
          <div
            className={`h-full rounded-full transition-[width] duration-500 ease-out ${color}`}
            style={{ width: `${Math.max(0, Math.min(100, percent))}%` }}
          />
        )}
      </div>

      <p className="mt-3 min-h-10 text-[12px] leading-relaxed text-zinc-600 dark:text-zinc-300">
        {progress?.detail || "HeaSpot đang xác định trình gỡ cài đặt phù hợp."}
      </p>

      <div className="mt-auto flex items-center justify-between gap-4 border-t border-black/5 pt-3 dark:border-white/10">
        <span className="text-[11px] text-zinc-500 dark:text-zinc-400">
          {progress?.done
            ? "Tác vụ đã kết thúc."
            : "Có thể thu nhỏ; tác vụ vẫn chạy và launcher vẫn dùng bình thường."}
        </span>
        {progress?.done && (
          <button
            type="button"
            autoFocus
            onClick={() => void getCurrentWindow().hide()}
            className="shrink-0 rounded-lg bg-zinc-800 px-4 py-2 text-[12px] font-semibold text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white"
          >
            Đóng
          </button>
        )}
      </div>
    </main>
  );
}
