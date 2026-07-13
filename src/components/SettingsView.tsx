import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import type { AppSettings } from "../types";
import { KW_FEATURES, resolveKeywords, DEFAULT_KEYWORDS, type KwMap } from "../keywords";
import { applyTheme } from "../theme";

const DEFAULTS: AppSettings = {
  search_hotkey: "Alt+Space",
  clipboard_hotkey: "Win+V",
  keywords: "{}",
  feature_hotkeys: "{}",
  max_clipboard_items: 500,
  clipboard_retention_days: 0,
  privacy_apps: "keepass,bitwarden,1password,lastpass,dashlane,protonpass",
  auto_paste: true,
  serper_api_key: "",
  enable_browser_passwords: false,
  password_to_history: false,
  theme: "system",
  launch_at_startup: false,
};

type Tab = "general" | "keywords" | "clipboard";

const GROUPS = ["Tìm kiếm & tri thức", "Tiện ích", "Hệ thống & điều hướng"] as const;

export function SettingsView({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("general");
  const [value, setValue] = useState<AppSettings>(DEFAULTS);
  const [kw, setKw] = useState<KwMap>(DEFAULT_KEYWORDS);
  const [featureHotkeys, setFeatureHotkeys] = useState<Record<string, string>>({});
  const [status, setStatus] = useState("");
  const [saving, setSaving] = useState(false);
  const mainRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [tab]);

  useEffect(() => {
    invoke<AppSettings>("get_settings")
      .then((s) => {
        setValue(s);
        setKw(resolveKeywords(s.keywords));
        try { setFeatureHotkeys(JSON.parse(s.feature_hotkeys || "{}")); } catch { setFeatureHotkeys({}); }
        applyTheme(s.theme);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [onClose]);

  // Cảnh báo keyword trùng nhau
  const duplicates = useMemo(() => {
    const seen = new Map<string, string>();
    const dup = new Set<string>();
    for (const f of KW_FEATURES) {
      const v = (kw[f.id] || "").toLowerCase().trim();
      if (!v) continue;
      if (seen.has(v)) {
        dup.add(f.id);
        dup.add(seen.get(v)!);
      } else seen.set(v, f.id);
    }
    return dup;
  }, [kw]);

  const save = async () => {
    if (duplicates.size > 0) {
      setStatus("⚠ Có keyword bị trùng — hãy sửa trước khi lưu");
      return;
    }
    setSaving(true);
    setStatus("Đang lưu…");
    try {
      const payload: AppSettings = {
        ...value,
        keywords: JSON.stringify(kw),
        feature_hotkeys: JSON.stringify(featureHotkeys),
      };
      await invoke("save_settings", { settings: payload });
      applyTheme(value.theme);
      setStatus("✓ Đã lưu cấu hình");
    } catch (e) {
      setStatus(String(e));
    } finally {
      setSaving(false);
    }
  };

  const resetKeywords = () => setKw({ ...DEFAULT_KEYWORDS });

  const field =
    "w-full rounded-lg border border-black/10 dark:border-white/10 bg-black/[.03] dark:bg-white/[.06] px-3 py-2 text-[13px] outline-none focus:border-blue-500/60";

  return (
    <div className="h-screen flex text-zinc-800 dark:text-zinc-100 bg-white dark:bg-zinc-900">
      <aside className="w-52 shrink-0 bg-black/[.035] dark:bg-black/25 border-r border-black/5 dark:border-white/10 p-3 flex flex-col">
        <div className="font-semibold px-2 py-3 text-[15px]">⚙ HeaSpot Settings</div>
        {(
          [
            ["general", "Chung"],
            ["keywords", "Keyword & Hotkey tính năng"],
            ["clipboard", "Clipboard & Bảo mật"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`w-full text-left px-3 py-2 rounded-lg text-[13px] mb-1 ${
              tab === id ? "bg-blue-600 text-white" : "hover:bg-black/5 dark:hover:bg-white/5"
            }`}
          >
            {label}
          </button>
        ))}
        <div className="mt-auto text-[10px] text-zinc-400 px-2">Cấu hình lưu trong SQLite của HeaSpot</div>
      </aside>

      <main className="flex-1 min-w-0 flex flex-col">
        <div ref={mainRef} className="flex-1 overflow-y-auto p-6 space-y-5">
          {tab === "general" && (
            <>
              <Section title="Hotkey mở nhanh">
                <label className="block">
                  <span className="block text-[12px] font-medium mb-1.5">Mở launcher (tìm kiếm)</span>
                  <HotkeyCapture value={value.search_hotkey} onChange={(hk) => setValue({ ...value, search_hotkey: hk })} />
                  <span className="block text-[10px] text-zinc-400 mt-1">Click rồi nhấn tổ hợp — cần ít nhất 1 phím Ctrl/Alt/Shift/Win. Backspace để xoá.</span>
                </label>
                <label className="block">
                  <span className="block text-[12px] font-medium mb-1.5">Mở Clipboard Manager</span>
                  <HotkeyCapture value={value.clipboard_hotkey} onChange={(hk) => setValue({ ...value, clipboard_hotkey: hk })} />
                  <span className="block text-[10px] text-zinc-400 mt-1"><b>Win+V</b> dùng hook đặc biệt để luôn thắng panel clipboard mặc định của Windows.</span>
                </label>
              </Section>

              <Section title="Giao diện">
                <label className="block">
                  <span className="block text-[12px] font-medium mb-1.5">Theme</span>
                  <select
                    className={field}
                    value={value.theme}
                    onChange={(e) => {
                      setValue({ ...value, theme: e.target.value });
                      applyTheme(e.target.value);
                    }}
                  >
                    <option value="system">Theo hệ thống</option>
                    <option value="light">Sáng</option>
                    <option value="dark">Tối</option>
                  </select>
                </label>
              </Section>

              <Section title="Kết quả nhanh Google (g)">
                <div className="rounded-lg border border-sky-500/30 bg-sky-500/10 p-3 text-[11.5px] text-sky-700 dark:text-sky-300">
                  Để <b>g &lt;từ khoá&gt;</b> phủ MỌI truy vấn (kể cả sản phẩm/tin niche), dán
                  Serper.dev API key — <b>miễn phí 2500 lượt, KHÔNG cần thẻ</b>. Lấy tại{" "}
                  <b>serper.dev</b> → Sign up (Google/email) → copy API key (trả kết quả Google thật).
                  Bỏ trống thì dùng DuckDuckGo + Wikipedia (chỉ phủ khái niệm phổ biến).
                </div>
                <label className="block">
                  <span className="block text-[12px] font-medium mb-1.5">Serper.dev API key</span>
                  <input className={field} type="password" placeholder="key…" value={value.serper_api_key}
                    onChange={(e) => setValue({ ...value, serper_api_key: e.target.value })} />
                </label>
              </Section>

              <Section title="Khởi động">
                <label className="flex items-center gap-3 text-[13px]">
                  <input type="checkbox" checked={value.launch_at_startup} onChange={(e) => setValue({ ...value, launch_at_startup: e.target.checked })} />
                  Tự khởi động HeaSpot cùng Windows
                </label>
              </Section>

              <Section title="Dọn cache toàn app">
                <div className="text-[11.5px] text-zinc-500 dark:text-zinc-400">
                  Xoá <b>toàn bộ cache tra cứu</b> của mọi tính năng trong RAM — dịch, tra nhanh,
                  Wikipedia, kết quả Google, full-text Windows Search, Capacities, icon app/file — và
                  các ảnh clip tạm mồ côi trên đĩa. <b>Không</b> đụng tới clipboard history, snippet
                  hay cấu hình đã lưu. Dành cho người dùng lâu ngày muốn dọn dẹp.
                </div>
                <button
                  onClick={async () => {
                    setStatus("Đang dọn cache…");
                    try {
                      const msg = await invoke<string>("clear_cache");
                      setStatus(`✓ ${msg}`);
                    } catch (e) {
                      setStatus(String(e));
                    }
                  }}
                  className="px-4 py-2 rounded-lg bg-amber-600 text-white text-[13px]"
                >
                  🧹 Dọn toàn bộ cache ngay
                </button>
              </Section>
            </>
          )}

          {tab === "keywords" && (
            <>
              <div className="flex items-center justify-between">
                <p className="text-[12px] text-zinc-500 max-w-lg">
                  Đổi từ khóa/ký tự kích hoạt từng tính năng. Gõ từ khóa này ở thanh tìm kiếm để dùng.
                  Để trống một ô sẽ khôi phục mặc định của tính năng đó.
                </p>
                <button onClick={resetKeywords} className="shrink-0 text-[12px] px-3 py-1.5 rounded-lg border border-black/10 dark:border-white/10 hover:bg-black/5 dark:hover:bg-white/5">
                  Khôi phục mặc định
                </button>
              </div>
              {GROUPS.map((group) => (
                <div key={group}>
                  <div className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400 mb-2">{group}</div>
                  <div className="grid grid-cols-2 gap-3">
                    {KW_FEATURES.filter((f) => f.group === group).map((f) => {
                      const dup = duplicates.has(f.id);
                      return (
                        <div key={f.id} className="p-3 rounded-lg bg-black/[.03] dark:bg-white/[.05]">
                          <div className="text-[12.5px] font-medium mb-1">{f.label}</div>
                          <div className="flex items-center gap-2">
                            <input
                              className={`w-24 shrink-0 rounded-md border px-2 py-1 text-[13px] font-mono outline-none bg-white dark:bg-zinc-800 ${
                                dup ? "border-red-500" : "border-black/10 dark:border-white/15 focus:border-blue-500/60"
                              }`}
                              value={kw[f.id] ?? ""}
                              placeholder={f.def}
                              onChange={(e) => setKw({ ...kw, [f.id]: e.target.value })}
                            />
                            <span className="text-[11px] text-zinc-400 truncate">
                              {f.kind === "prefix" ? "ký tự đầu" : "từ khóa"} · vd: {f.example}
                            </span>
                          </div>
                          <div className="mt-2 grid grid-cols-[4rem_minmax(0,1fr)] items-center gap-x-2 gap-y-1">
                            <span className="text-[10px] text-zinc-400 w-16 shrink-0">Hotkey</span>
                            <HotkeyCapture
                              value={featureHotkeys[f.id] || ""}
                              onChange={(hotkey) => setFeatureHotkeys({ ...featureHotkeys, [f.id]: hotkey })}
                            />
                            <span className="col-span-2 text-[10px] leading-relaxed text-zinc-400">Bôi đen text → nhấn hotkey để mở thẳng</span>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
              {duplicates.size > 0 && (
                <div className="text-[12px] text-red-500">⚠ Có keyword bị trùng (viền đỏ) — mỗi tính năng phải có keyword khác nhau.</div>
              )}
            </>
          )}

          {tab === "clipboard" && (
            <>
              <Section title="Hành vi">
                <label className="flex items-center gap-3 text-[13px]">
                  <input type="checkbox" checked={value.auto_paste} onChange={(e) => setValue({ ...value, auto_paste: e.target.checked })} />
                  Tự động dán (Enter dán thẳng vào cửa sổ trước) — tắt để chỉ copy
                </label>
              </Section>
              <Section title="Lưu trữ">
                <label className="block">
                  <span className="block text-[12px] font-medium mb-1.5">Thời gian lưu lịch sử</span>
                  <select className={field} value={value.clipboard_retention_days} onChange={(e) => setValue({ ...value, clipboard_retention_days: Number(e.target.value) })}>
                    <option value={0}>Giữ mãi (chỉ giới hạn theo số lượng)</option>
                    <option value={1}>1 ngày</option>
                    <option value={7}>7 ngày</option>
                    <option value={30}>30 ngày</option>
                    <option value={90}>90 ngày</option>
                    <option value={365}>1 năm</option>
                  </select>
                  <span className="text-[10px] text-zinc-500">Item đã ghim không bao giờ bị xóa tự động.</span>
                </label>
                <label className="block">
                  <span className="block text-[12px] font-medium mb-1.5">Số item chưa ghim tối đa (50–2000)</span>
                  <input className={field} type="number" min={50} max={2000} value={value.max_clipboard_items} onChange={(e) => setValue({ ...value, max_clipboard_items: Number(e.target.value) })} />
                </label>
              </Section>
              <Section title="Quyền riêng tư (Privacy Guard)">
                <label className="block">
                  <span className="block text-[12px] font-medium mb-1.5">Không lưu clipboard từ các process này</span>
                  <textarea className={`${field} h-24 resize-none font-mono`} value={value.privacy_apps} onChange={(e) => setValue({ ...value, privacy_apps: e.target.value })} />
                  <span className="text-[10px] text-zinc-500">Phân tách bằng dấu phẩy hoặc xuống dòng. Cache ảnh mồ côi được tự dọn khi app khởi động.</span>
                </label>
              </Section>

              <Section title="Mật khẩu trình duyệt (nhạy cảm)">
                <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-[11.5px] text-amber-700 dark:text-amber-300">
                  ⚠ Khi bật, gõ <b>pw &lt;từ khóa&gt;</b> để tìm mật khẩu đã lưu trong Edge/Chrome/Brave… của
                  chính tài khoản Windows này. Chỉ hoạt động trên máy bạn. Hãy đảm bảo không ai khác dùng chung máy.
                </div>
                <label className="flex items-center gap-3 text-[13px]">
                  <input type="checkbox" checked={value.enable_browser_passwords} onChange={(e) => setValue({ ...value, enable_browser_passwords: e.target.checked })} />
                  Cho phép đọc &amp; tìm mật khẩu đã lưu trong trình duyệt
                </label>
                <label className="flex items-center gap-3 text-[13px]">
                  <input type="checkbox" checked={value.password_to_history} onChange={(e) => setValue({ ...value, password_to_history: e.target.checked })} />
                  Khi copy mật khẩu thì <b className="mx-1">lưu</b> vào clipboard history (mặc định KHÔNG lưu cho an toàn)
                </label>
              </Section>
            </>
          )}
        </div>

        <div className="flex items-center justify-between px-6 py-3 border-t border-black/5 dark:border-white/10">
          <span className={`text-[12px] ${status.startsWith("⚠") ? "text-red-500" : "text-zinc-500"}`}>{status}</span>
          <div className="flex gap-2">
            <button onClick={onClose} className="px-4 py-2 rounded-lg text-[13px] border border-black/10 dark:border-white/10 hover:bg-black/5 dark:hover:bg-white/5">
              Đóng
            </button>
            <button onClick={save} disabled={saving} className="px-5 py-2 rounded-lg bg-blue-600 text-white text-[13px] disabled:opacity-60">
              Lưu thay đổi
            </button>
          </div>
        </div>
      </main>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-3">
      <div className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">{title}</div>
      {children}
    </div>
  );
}

function HotkeyCapture({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <input
      readOnly
      value={value}
      placeholder="Click rồi nhấn phím"
      title="Backspace/Delete để xóa hotkey"
      className="w-full min-w-0 rounded-md border border-black/10 dark:border-white/15 bg-white dark:bg-zinc-800 px-2 py-1 text-[11px] font-mono outline-none focus:border-blue-500/60"
      onKeyDown={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Backspace" || e.key === "Delete") { onChange(""); return; }
        if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
        const mods = [e.ctrlKey ? "Ctrl" : "", e.altKey ? "Alt" : "", e.shiftKey ? "Shift" : "", e.metaKey ? "Win" : ""].filter(Boolean);
        if (mods.length === 0) return;
        const key = e.key === " " ? "Space" : e.key.length === 1 ? e.key.toUpperCase() : e.key;
        onChange([...mods, key].join("+"));
      }}
    />
  );
}
