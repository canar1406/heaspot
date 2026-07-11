# WinSpot

Spotlight / Alfred cho Windows — ứng dụng launcher siêu nhẹ xây dựng bằng **Tauri (Rust + React)**.

## Phím tắt toàn cục

| Phím | Chức năng |
|---|---|
| `Alt + Space` | Mở / ẩn thanh tìm kiếm |
| `Win + V` (hoặc `Ctrl + Shift + V`) | Mở Clipboard Manager kiểu Alfred |

## Cú pháp trên thanh search

| Gõ | Kết quả |
|---|---|
| `chrome` | Tìm và mở app / file (Enter để mở) |
| `100 * 20%` | Quick Calculator — Enter để copy kết quả |
| `g <từ khoá>` | Tìm Google · `yt` YouTube · `wiki` Wikipedia |
| `in <từ khoá>` hoặc `doc:<từ khoá>` | Full-text search nội dung file (Word, PDF, Excel...) qua Windows Search **+ note Capacities** |
| `capacities token <token>` | Kết nối Capacities (lấy token: Capacities → Settings → Capacities API) |
| `> <lệnh>` | Chạy lệnh trong cửa sổ Terminal mới |
| `;` | Liệt kê Snippets — Enter để copy nội dung |
| `snip <keyword> <nội dung>` | Lưu snippet mới (VD: `snip mail hello@myemail.com`) |
| `sleep` / `shutdown` / `restart` / `lock` / `mute` / `empty trash` | Lệnh hệ thống |
| `< <tên>` | **Window Walker** — chuyển focus sang cửa sổ đang mở (thay Alt+Tab) |
| `{ <tên>` | Mở project/workspace gần đây của **VS Code** |
| `! <tên>` | Tìm **Windows Services** — bấm `→` để Start/Stop/Restart (admin) |
| `: hkcu\...` | Duyệt **Registry**, Enter mở Registry Editor đúng key |
| `# uuid` / `# md5 text` / `# sha256 text` / `# b64 text` | **Value Generator** |
| `10 ft to m` / `100 f to c` / `2 gb to mb` | **Unit Converter** |
| `time tokyo` / `time gmt+7` | Giờ thế giới |
| `github.com` / `a@b.com` | Nhận diện URL/email, mở ngay |
| `=sin(pi/2)` / `100*(50+20)/2` | Máy tính nâng cao (hàm lượng giác, hằng số pi, e) |
| `workflow` / `workflow install <path>` | **Alfred Workflows** — xem/cài plugin Alfred |

Điều hướng: `↑ ↓` chọn · `Enter` mở · **`→` mở Context Menu** (Run as admin, Open file location, Copy path, Run in Terminal...) · `←` đóng menu · `Tab` điền tên · `Ctrl+1..9` mở nhanh · `Esc` xoá/đóng.

### Alfred Workflow Compatibility

File `.alfredworkflow` bản chất là ZIP chứa `info.plist` + script. WinSpot hỗ trợ nhóm **Script Filter** viết bằng Python / Node.js / PHP / Ruby / Bash (cần cài sẵn runtime tương ứng trong PATH). Workflow dùng AppleScript sẽ báo không hỗ trợ.

- Cài: gõ `workflow install C:\đường\dẫn\xyz.alfredworkflow`
- Dùng: gõ `<keyword> <từ khoá>` như trên Alfred; kết quả (arg) là URL thì mở, là đường dẫn thì mở file, còn lại copy.
- Workflow được giải nén vào `%APPDATA%\winspot\workflows\`.

### Clipboard Manager (Win+V)

Giao diện 2 cột kiểu Alfred: danh sách bên trái, **preview chỉnh sửa được** bên phải.

- `↑ ↓` chọn mục · gõ để lọc · `Enter` copy (bản đã sửa) rồi đóng
- `Tab` / `Ctrl+E` nhảy vào khung edit · sửa thoải mái · `Ctrl+S` lưu · `Ctrl+Enter` copy & đóng · `Esc` quay lại
- `Ctrl+D` xoá mục · `Ctrl+L` xoá tất cả

> `Win+V` được bắt bằng low-level keyboard hook (WH_KEYBOARD_LL) nên **luôn thắng** panel clipboard mặc định của Windows, kể cả khi Windows Clipboard History đang bật. `Ctrl+Shift+V` là phím phụ dự phòng.

## Kiến trúc

- **Frontend** (`src/`): React + TypeScript + Tailwind CSS + Framer Motion.
- **Backend** (`src-tauri/`): Rust — global hotkey, index app (Start Menu + Registry App Paths), index file, clipboard watcher, SQLite (`%APPDATA%\winspot\winspot.db`), hiệu ứng Mica/Acrylic, tray icon.
- **File search**: **Everything được bundle sẵn trong app** (`resources/everything/`). Khi khởi động, WinSpot tự chạy `Everything.exe -startup` chạy nền nếu chưa có instance nào, và query qua `Everything64.dll` (SDK IPC). Nếu Everything không hoạt động được thì fallback sang index nội bộ (Desktop, Documents, Downloads, Pictures, Videos, Music).
- **Full-text search**: query OLE DB `Search.CollatorDSO` của Windows Search Index, đồng thời tìm trong **note Capacities** qua API chính thức (`api.capacities.io`) nếu đã lưu token — kết quả mở thẳng note bằng deep link `capacities://`.
- **Icon app**: trích xuất icon thật của từng app (.lnk/.exe) bằng Shell API (`SHGetFileInfoW` → PNG data URL), có cache theo path.

## Phát triển

```bash
npm install
npm run tauri dev
```

Yêu cầu: Node.js, Rust (MSVC toolchain), Visual Studio Build Tools (C++), WebView2 (có sẵn trên Windows 11).

## Đóng gói .exe

```bash
npm run tauri build
```

File cài đặt NSIS nằm tại `src-tauri/target/release/bundle/nsis/`.

## Ghi chú

- Clipboard history hiện lưu **text và link** (ảnh chưa hỗ trợ).
- Full-text search chỉ thấy các file nằm trong phạm vi Windows Search đã index (Settings → Searching Windows).
- Everything ([voidtools](https://www.voidtools.com/)) đã được bundle sẵn và tự chạy nền — không cần cài riêng. Lần chạy đầu, Everything có thể hỏi quyền/đề nghị cài Everything Service để index NTFS toàn ổ đĩa (một lần duy nhất); nếu bạn đã cài Everything riêng thì WinSpot dùng luôn instance đó.
