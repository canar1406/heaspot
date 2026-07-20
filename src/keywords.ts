/**
 * Trung tâm cấu hình KEYWORD/CÚ PHÁP kích hoạt từng tính năng.
 * Người dùng đổi được keyword trong Settings; router (useSearch) đọc map này.
 *
 * - kind "word":   gõ nguyên từ + khoảng trắng, VD `in báo cáo`, `tr hello`.
 * - kind "prefix": ký tự đứng đầu, VD `<edge`, `>ping`, `;mail`.
 */
export type KwKind = "word" | "prefix";

export interface KwFeature {
  id: string;
  label: string;
  def: string;
  kind: KwKind;
  example: string;
  group: "Tìm kiếm & tri thức" | "Tiện ích" | "Hệ thống & điều hướng";
}

export const KW_FEATURES: KwFeature[] = [
  // Tìm kiếm & tri thức
  { id: "fulltext", label: "Tìm nội dung trong file / Capacities", def: "in", kind: "word", example: "in kế hoạch marketing", group: "Tìm kiếm & tri thức" },
  { id: "translate", label: "Dịch + từ điển học thuật", def: "tr", kind: "word", example: "tr resilience", group: "Tìm kiếm & tri thức" },
  { id: "wiki", label: "Tra Wikipedia", def: "wiki", kind: "word", example: "wiki trí tuệ nhân tạo", group: "Tìm kiếm & tri thức" },
  { id: "review", label: "Danh sách từ ôn tập", def: "review", kind: "word", example: "review", group: "Tìm kiếm & tri thức" },
  { id: "formula", label: "Công thức Toán/Lý/Hóa", def: "formula", kind: "word", example: "formula động năng", group: "Tìm kiếm & tri thức" },
  { id: "chemistry", label: "Từ điển hóa học (nguyên tố, phân tử khối)", def: "chem", kind: "word", example: "chem Fe · chem H2O", group: "Tìm kiếm & tri thức" },
  { id: "latex", label: "Công thức LaTeX (căn, phân số, ma trận…)", def: "latex", kind: "word", example: "latex hệ phương trình", group: "Tìm kiếm & tri thức" },
  { id: "google", label: "Tìm Google", def: "g", kind: "word", example: "g cách làm bánh", group: "Tìm kiếm & tri thức" },
  { id: "youtube", label: "Tìm YouTube", def: "yt", kind: "word", example: "yt lofi", group: "Tìm kiếm & tri thức" },
  // Tiện ích
  { id: "ocr", label: "Chụp màn hình & nhận chữ (OCR)", def: "ocr", kind: "word", example: "ocr", group: "Tiện ích" },
  { id: "emoji", label: "Tìm Emoji thông minh", def: "emoji", kind: "word", example: "emoji cảm ơn", group: "Tiện ích" },
  { id: "convert", label: "Đổi đơn vị (dài, khối lượng, tốc độ, áp suất…) / cơ số", def: "conv", kind: "word", example: "conv 60 km/h to m/s", group: "Tiện ích" },
  { id: "time", label: "Giờ thế giới", def: "time", kind: "word", example: "time tokyo", group: "Tiện ích" },
  { id: "url", label: "Mở URL", def: "url", kind: "word", example: "url github.com", group: "Tiện ích" },
  { id: "password", label: "Mật khẩu trình duyệt (bật trong Settings)", def: "pw", kind: "word", example: "pw github", group: "Tiện ích" },
  { id: "generator", label: "Tạo UUID / hash / base64", def: "#", kind: "prefix", example: "#uuid", group: "Tiện ích" },
  { id: "snippet", label: "Snippet gõ tắt", def: ";", kind: "prefix", example: ";mail", group: "Tiện ích" },
  // Dev tools
  { id: "json", label: "Format / kiểm tra JSON (từ clipboard)", def: "json", kind: "word", example: "json", group: "Tiện ích" },
  { id: "jwt", label: "Giải mã JWT token", def: "jwt", kind: "word", example: "jwt eyJhbGci…", group: "Tiện ích" },
  { id: "port", label: "Tìm & kill tiến trình chiếm cổng TCP", def: "port", kind: "word", example: "port 3000", group: "Hệ thống & điều hướng" },
  // Hệ thống & điều hướng
  { id: "process", label: "Task Manager — tiến trình đang chạy", def: "ps", kind: "word", example: "ps chrome", group: "Hệ thống & điều hướng" },
  { id: "system", label: "Lệnh hệ thống (shutdown, lock…)", def: "sys", kind: "word", example: "sys shutdown", group: "Hệ thống & điều hướng" },
  { id: "window", label: "Chuyển cửa sổ (Window Walker)", def: "<", kind: "prefix", example: "<edge", group: "Hệ thống & điều hướng" },
  { id: "vscode", label: "Mở project VS Code gần đây", def: "{", kind: "prefix", example: "{winspot", group: "Hệ thống & điều hướng" },
  { id: "service", label: "Windows Services", def: "!", kind: "prefix", example: "!print", group: "Hệ thống & điều hướng" },
  { id: "registry", label: "Duyệt Registry", def: ":", kind: "prefix", example: ":hkcu\\software", group: "Hệ thống & điều hướng" },
  { id: "terminal", label: "Chạy lệnh Terminal", def: ">", kind: "prefix", example: ">ping google.com", group: "Hệ thống & điều hướng" },
];

export type KwMap = Record<string, string>;

export const DEFAULT_KEYWORDS: KwMap = Object.fromEntries(
  KW_FEATURES.map((f) => [f.id, f.def])
);

/** Gộp default với override do người dùng lưu (bỏ qua giá trị rỗng) */
export function resolveKeywords(json: string | undefined | null): KwMap {
  const map: KwMap = { ...DEFAULT_KEYWORDS };
  if (json) {
    try {
      const parsed = JSON.parse(json) as Record<string, unknown>;
      for (const f of KW_FEATURES) {
        const v = parsed[f.id];
        if (typeof v === "string" && v.trim()) map[f.id] = v.trim();
      }
    } catch {
      /* giữ default */
    }
  }
  return map;
}

/** Thoát ký tự đặc biệt để nhúng keyword vào RegExp an toàn */
export function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Khớp keyword dạng "word": trả về phần argument phía sau (đã trim),
 * hoặc "" nếu chỉ gõ đúng keyword, hoặc null nếu không khớp.
 */
export function matchWord(query: string, keyword: string): string | null {
  if (!keyword) return null;
  const re = new RegExp(`^${escapeRe(keyword)}(?:\\s+(.*))?$`, "i");
  const m = re.exec(query.trim());
  return m ? (m[1] ?? "").trim() : null;
}
