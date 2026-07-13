import type { ResultItemData } from "../types";

interface SysCmd {
  action: string;
  title: string;
  subtitle: string;
  keywords: string[];
}

const COMMANDS: SysCmd[] = [
  {
    action: "sleep",
    title: "Sleep",
    subtitle: "Đưa máy vào chế độ ngủ",
    keywords: ["sleep", "ngu", "ngủ"],
  },
  {
    action: "shutdown",
    title: "Shutdown",
    subtitle: "Tắt máy ngay lập tức",
    keywords: ["shutdown", "tat may", "tắt máy"],
  },
  {
    action: "restart",
    title: "Restart",
    subtitle: "Khởi động lại máy",
    keywords: ["restart", "reboot", "khoi dong lai"],
  },
  {
    action: "lock",
    title: "Lock",
    subtitle: "Khoá màn hình",
    keywords: ["lock", "khoa", "khoá màn hình"],
  },
  {
    action: "empty-trash",
    title: "Empty Trash",
    subtitle: "Dọn sạch Recycle Bin",
    keywords: ["empty trash", "trash", "recycle", "don rac", "xóa rác"],
  },
  {
    action: "mute",
    title: "Mute",
    subtitle: "Bật / tắt tiếng hệ thống",
    keywords: ["mute", "tat tieng", "tắt tiếng", "im lang"],
  },
];

/** Lọc lệnh hệ thống theo query. Query rỗng (chỉ gõ keyword + Space) -> gợi ý TẤT CẢ. */
export function matchSystemCommands(query: string): ResultItemData[] {
  const q = query.trim().toLowerCase();
  const toItem = (c: SysCmd): ResultItemData => ({
    id: `system:${c.action}`,
    title: c.title,
    subtitle: c.subtitle,
    kind: "system" as const,
    action: c.action,
  });
  if (!q) return COMMANDS.map(toItem);
  return COMMANDS.filter((c) =>
    c.keywords.some((k) => k.startsWith(q) || q.startsWith(k))
  ).map(toItem);
}
