import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

type UpdateProgress = {
  stage: string;
  detail: string;
  version: string | null;
  notes: string;
  percent: number | null;
  available: boolean;
  installing: boolean;
  done: boolean;
  success: boolean;
};

export function UpdateProgressView() {
  const [progress, setProgress] = useState<UpdateProgress>();
  const [error, setError] = useState("");

  useEffect(() => {
    const sync = () => void invoke<UpdateProgress | null>("get_update_progress")
      .then((value) => value && setProgress(value)).catch(() => {});
    sync();
    const poll = window.setInterval(sync, 750);
    const unlisten = listen<UpdateProgress>("update://progress", (event) => setProgress(event.payload));
    return () => {
      window.clearInterval(poll);
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  const percent = progress?.percent;
  return (
    <main className="flex h-full flex-col bg-zinc-50 px-5 py-4 text-zinc-900 dark:bg-zinc-900 dark:text-zinc-50">
      <div className="flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-blue-600 text-xl text-white">↥</div>
        <div className="min-w-0 flex-1">
          <h1 className="text-[16px] font-semibold">{progress?.stage || "Cập nhật HeaSpot"}</h1>
          <p className="mt-1 text-[12px] text-zinc-500 dark:text-zinc-400">{progress?.detail || "Đang chuẩn bị…"}</p>
        </div>
        {percent !== null && percent !== undefined && <span className="text-[13px] font-semibold tabular-nums">{percent}%</span>}
      </div>
      <div className="mt-4 h-2.5 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-700">
        {percent === null || percent === undefined ? (
          <div className="h-full w-1/3 animate-pulse rounded-full bg-blue-500" />
        ) : (
          <div className="h-full rounded-full bg-blue-500 transition-[width] duration-500" style={{ width: `${percent}%` }} />
        )}
      </div>
      {progress?.notes && (
        <div className="mt-4 max-h-24 overflow-y-auto whitespace-pre-wrap rounded-lg bg-black/5 p-3 text-[11.5px] leading-relaxed text-zinc-600 dark:bg-white/5 dark:text-zinc-300">
          {progress.notes}
        </div>
      )}
      {error && <div className="mt-3 rounded-lg bg-red-500/10 p-2 text-[12px] text-red-600 dark:text-red-300">{error}</div>}
      <div className="mt-auto flex items-center justify-between border-t border-black/5 pt-3 dark:border-white/10">
        <span className="text-[11px] text-zinc-500">Gói cập nhật luôn được kiểm tra chữ ký trước khi cài.</span>
        <div className="flex gap-2">
          {!progress?.installing && (
            <button type="button" onClick={() => void getCurrentWindow().hide()}
              className="rounded-lg border border-black/10 px-4 py-2 text-[12px] dark:border-white/10">Để sau</button>
          )}
          {progress?.available && !progress.installing && (
            <button type="button" onClick={async () => {
              setError("");
              try { await invoke("install_available_update"); } catch (e) { setError(String(e)); }
            }} className="rounded-lg bg-blue-600 px-4 py-2 text-[12px] font-semibold text-white">
              Cập nhật &amp; khởi động lại
            </button>
          )}
        </div>
      </div>
    </main>
  );
}
