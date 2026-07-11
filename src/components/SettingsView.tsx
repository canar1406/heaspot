import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { AppSettings } from "../types";

const DEFAULTS: AppSettings = {
  search_hotkey: "Alt+Space", clipboard_hotkey: "Win+V", max_clipboard_items: 500,
  clipboard_retention_days: 0,
  privacy_apps: "keepass,bitwarden,1password,lastpass,dashlane,protonpass", launch_at_startup: false,
};

export function SettingsView({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<"general" | "clipboard" | "features">("general");
  const [value, setValue] = useState<AppSettings>(DEFAULTS);
  const [status, setStatus] = useState("");
  useEffect(() => { invoke<AppSettings>("get_settings").then(setValue).catch(() => {}); }, []);
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", key); return () => window.removeEventListener("keydown", key);
  }, [onClose]);
  const save = async () => {
    setStatus("Đang lưu…");
    try { await invoke("save_settings", { settings: value }); setStatus("Đã lưu cấu hình"); }
    catch (e) { setStatus(String(e)); }
  };
  const field = "w-full rounded-lg border border-black/10 dark:border-white/10 bg-black/[.03] dark:bg-white/[.06] px-3 py-2 text-[13px] outline-none";
  return (
    <div className="h-[560px] flex text-zinc-800 dark:text-zinc-100">
      <aside className="w-48 shrink-0 bg-black/[.035] dark:bg-black/20 border-r border-black/5 dark:border-white/10 p-3">
        <div className="font-semibold px-2 py-3">WinSpot Settings</div>
        {([['general','General'],['clipboard','Clipboard & Privacy'],['features','Cú pháp & tính năng']] as const).map(([id,label]) =>
          <button key={id} onClick={() => setTab(id)} className={`w-full text-left px-3 py-2 rounded-lg text-[13px] mb-1 ${tab===id?'bg-blue-600 text-white':'hover:bg-black/5 dark:hover:bg-white/5'}`}>{label}</button>)}
      </aside>
      <main className="flex-1 min-w-0 flex flex-col">
        <div className="flex items-center justify-between px-6 py-4 border-b border-black/5 dark:border-white/10">
          <div><div className="font-semibold">{tab === 'general' ? 'General' : tab === 'clipboard' ? 'Clipboard & Privacy' : 'Cú pháp nhanh'}</div><div className="text-[11px] text-zinc-500">Cấu hình được lưu trong SQLite của WinSpot</div></div>
          <button onClick={onClose} className="text-xl text-zinc-400 hover:text-zinc-700">×</button>
        </div>
        <div className="flex-1 overflow-y-auto p-6 space-y-5">
          {tab === 'general' && <>
            <label className="block"><span className="block text-[12px] font-medium mb-1.5">Hotkey mở launcher</span><select className={field} value={value.search_hotkey} onChange={e=>setValue({...value,search_hotkey:e.target.value})}>{['Alt+Space','Ctrl+Space','Ctrl+Alt+Space','Alt+F1'].map(x=><option key={x}>{x}</option>)}</select></label>
            <label className="block"><span className="block text-[12px] font-medium mb-1.5">Hotkey mở Clipboard</span><select className={field} value={value.clipboard_hotkey} onChange={e=>setValue({...value,clipboard_hotkey:e.target.value})}>{['Win+V','Ctrl+Shift+V','Alt+V','Ctrl+Alt+V'].map(x=><option key={x}>{x}</option>)}</select></label>
            <label className="flex items-center gap-3 text-[13px]"><input type="checkbox" checked={value.launch_at_startup} onChange={e=>setValue({...value,launch_at_startup:e.target.checked})}/> Tự khởi động WinSpot cùng Windows</label>
          </>}
          {tab === 'clipboard' && <>
            <label className="block"><span className="block text-[12px] font-medium mb-1.5">Thời gian lưu lịch sử</span><select className={field} value={value.clipboard_retention_days} onChange={e=>setValue({...value,clipboard_retention_days:Number(e.target.value)})}><option value={0}>Giữ mãi (chỉ giới hạn theo số lượng)</option><option value={1}>1 ngày</option><option value={7}>7 ngày</option><option value={30}>30 ngày</option><option value={90}>90 ngày</option><option value={365}>1 năm</option></select><span className="text-[10px] text-zinc-500">Item đã ghim không bao giờ bị xóa tự động.</span></label>
            <label className="block"><span className="block text-[12px] font-medium mb-1.5">Số item chưa ghim tối đa (50–2000)</span><input className={field} type="number" min={50} max={2000} value={value.max_clipboard_items} onChange={e=>setValue({...value,max_clipboard_items:Number(e.target.value)})}/></label>
            <label className="block"><span className="block text-[12px] font-medium mb-1.5">Không lưu clipboard từ các process này</span><textarea className={`${field} h-28 resize-none`} value={value.privacy_apps} onChange={e=>setValue({...value,privacy_apps:e.target.value})}/><span className="text-[10px] text-zinc-500">Phân tách bằng dấu phẩy hoặc xuống dòng. Cache ảnh mồ côi được tự dọn khi app khởi động.</span></label>
          </>}
          {tab === 'features' && <div className="grid grid-cols-2 gap-3 text-[12px]">
            {['wiki trí tuệ nhân tạo — tra Wikipedia','tr hello — dịch + word forms','255 dec to hex — đổi cơ số','ascii Hello to bin — ASCII/binary','< edge — Window Walker','{ project — VS Code recent','! service — Windows Services',': hkcu\\software — Registry','> command — Terminal',';keyword — Snippets'].map(x=><div key={x} className="p-3 rounded-lg bg-black/[.03] dark:bg-white/[.05]">{x}</div>)}
          </div>}
        </div>
        <div className="flex items-center justify-between px-6 py-3 border-t border-black/5 dark:border-white/10"><span className="text-[11px] text-zinc-500">{status}</span><button onClick={save} className="px-5 py-2 rounded-lg bg-blue-600 text-white text-[13px]">Lưu thay đổi</button></div>
      </main>
    </div>
  );
}
