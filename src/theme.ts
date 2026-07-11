/** Áp dụng theme thủ công (system/light/dark) — tailwind darkMode: "class". */
let mql: MediaQueryList | null = null;
let mqlHandler: ((e: MediaQueryListEvent) => void) | null = null;

export function applyTheme(theme: string) {
  const root = document.documentElement;
  const setDark = (on: boolean) => {
    root.classList.toggle("dark", on);
    // Cho native control (dropdown <select>, checkbox, scrollbar) render đúng tông
    root.style.colorScheme = on ? "dark" : "light";
  };

  // Gỡ listener cũ nếu có
  if (mql && mqlHandler) {
    mql.removeEventListener("change", mqlHandler);
    mql = null;
    mqlHandler = null;
  }

  if (theme === "dark") {
    setDark(true);
  } else if (theme === "light") {
    setDark(false);
  } else {
    // system: theo prefers-color-scheme và cập nhật khi hệ thống đổi
    mql = window.matchMedia("(prefers-color-scheme: dark)");
    setDark(mql.matches);
    mqlHandler = (e) => setDark(e.matches);
    mql.addEventListener("change", mqlHandler);
  }
  root.setAttribute("data-theme", theme);
}
