/**
 * Time & Date: "time", "time tokyo", "time gmt+7", "date new york"
 */
import type { ResultItemData } from "../types";

const ZONES: Record<string, string> = {
  tokyo: "Asia/Tokyo",
  hanoi: "Asia/Ho_Chi_Minh",
  "ha noi": "Asia/Ho_Chi_Minh",
  vietnam: "Asia/Ho_Chi_Minh",
  saigon: "Asia/Ho_Chi_Minh",
  "ho chi minh": "Asia/Ho_Chi_Minh",
  london: "Europe/London",
  paris: "Europe/Paris",
  berlin: "Europe/Berlin",
  moscow: "Europe/Moscow",
  dubai: "Asia/Dubai",
  delhi: "Asia/Kolkata",
  india: "Asia/Kolkata",
  bangkok: "Asia/Bangkok",
  singapore: "Asia/Singapore",
  beijing: "Asia/Shanghai",
  shanghai: "Asia/Shanghai",
  "hong kong": "Asia/Hong_Kong",
  seoul: "Asia/Seoul",
  sydney: "Australia/Sydney",
  "new york": "America/New_York",
  newyork: "America/New_York",
  ny: "America/New_York",
  chicago: "America/Chicago",
  seattle: "America/Los_Angeles",
  la: "America/Los_Angeles",
  "los angeles": "America/Los_Angeles",
  sf: "America/Los_Angeles",
  utc: "UTC",
};

function formatIn(timeZone: string): string {
  return new Intl.DateTimeFormat("vi-VN", {
    timeZone,
    hour: "2-digit",
    minute: "2-digit",
    weekday: "long",
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  }).format(new Date());
}

export function tryTime(q: string): ResultItemData | null {
  const m = /^(?:time|date|gio|giờ)\s*(.*)$/i.exec(q.trim());
  if (!m) return null;
  const rest = m[1].trim().toLowerCase();

  let zone: string | null = null;
  let label = "";

  if (!rest) {
    zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
    label = `Giờ địa phương (${zone})`;
  } else {
    const gmt = /^(?:gmt|utc)\s*([+-]\d{1,2})$/.exec(rest);
    if (gmt) {
      const offset = parseInt(gmt[1], 10);
      const d = new Date(Date.now() + offset * 3600_000);
      const title = d.toISOString().replace("T", " ").slice(0, 16);
      return {
        id: `time:${rest}`,
        title: `${title} (GMT${gmt[1]})`,
        subtitle: "Time & Date — Enter để copy",
        kind: "time",
        text: title,
      };
    }
    zone = ZONES[rest] ?? null;
    label = rest;
    if (!zone) return null;
  }

  const title = formatIn(zone);
  return {
    id: `time:${rest || "local"}`,
    title,
    subtitle: `${label} — Enter để copy`,
    kind: "time",
    text: title,
  };
}
