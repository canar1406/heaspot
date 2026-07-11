# HeaSpot

**HeaSpot** — launcher kiểu **Spotlight (macOS) / Alfred** cho Windows, xây bằng **Tauri (Rust + React)**. Siêu nhẹ (~10 MB RAM khi ẩn), keyboard-first, mở tức thì.

## Phím tắt toàn cục

| Phím | Chức năng |
|---|---|
| `Alt + Space` | Mở / ẩn thanh tìm kiếm |
| `Win + V` (hoặc `Ctrl + Shift + V`) | Mở Clipboard Manager kiểu Alfred |

> `Win+V` được bắt bằng low-level keyboard hook nên luôn thắng panel clipboard mặc định của Windows. Nếu `Alt+Space` bị PowerToys Run… chiếm, HeaSpot tự chuyển sang hook để vẫn hoạt động. Đổi hotkey trong Settings.

## Cú pháp keyword (đổi được trong Settings → Keyword / Cú pháp)

| Gõ | Kết quả |
|---|---|
| `chrome` | Tìm & mở app / file / folder (icon thật) |
| `in <từ khoá>` / `doc:<từ khoá>` | Tìm **nội dung trong file** (Word/PDF/Excel…) qua Windows Search **+ note Capacities** |
| `tr <từ>` | **Dịch + từ điển học thuật** (IPA, ví dụ, đồng/trái nghĩa, lưu vào danh sách ôn tập) |
| `wiki <khái niệm>` | Tra Wikipedia (preview) |
| `chem <ký hiệu/tên/Z>` | **Từ điển hóa học**: số proton, nguyên tử khối, phân loại. VD `chem Fe`, `chem oxy`, `chem 26` |
| `chem <công thức>` | **Phân tử khối**: `chem H2O`, `chem Ca(OH)2`, `chem C6H12O6` |
| `formula <tên>` | Công thức Toán / Lý / Hóa |
| `review` | Danh sách từ đã lưu để ôn tập |
| `= 100*(50+20)/2` / `=sin(pi/2)` | Máy tính nâng cao (lượng giác, hằng số) |
| `conv 10 ft to m` / `conv 255 dec to hex` | Đổi đơn vị / cơ số / ASCII |
| `time tokyo` / `time gmt+7` | Giờ thế giới |
| `g <từ khoá>` · `yt <từ khoá>` | Tìm Google / YouTube |
| `url github.com` | Mở URL |
| `#uuid` · `#md5 text` · `#b64 text` | Tạo UUID / hash / base64 |
| `ps <tên>` | **Task Manager** — tiến trình đang chạy (→ để **Kill** mạnh) |
| `pw <từ khoá>` | **Mật khẩu trình duyệt** đã lưu (bật trong Settings) |
| `< <tên>` | **Window Walker** — chuyển cửa sổ đang mở |
| `{ <tên>` | Mở project VS Code gần đây |
| `! <tên>` | Windows Services (→ Start/Stop/Restart) |
| `: hkcu\...` | Duyệt Registry |
| `> <lệnh>` | Chạy lệnh Terminal |
| `; ` | Snippets gõ tắt; thêm bằng `snip <keyword> <nội dung>` |
| `sys shutdown` / `sleep` / `lock` / `mute`… | Lệnh hệ thống |
| `settings` | Mở cửa sổ Cài đặt |
| `workflow` / `workflow install <path>` | Alfred Workflow (Python/Node/PHP/Ruby/Bash) |

Điều hướng: `↑ ↓` chọn · `Enter` mở · **`→` mở Context Menu** (Run as admin, Open location, Copy path, Kill…) · `Tab` điền · `Ctrl+1..9` mở nhanh · `Esc` đóng.

## Clipboard Manager (Win+V)

Giao diện 2 cột kiểu Alfred: danh sách bên trái, **preview chỉnh sửa trực tiếp** bên phải.

- Lưu **text, link, ảnh (thumbnail + preview), danh sách file** — dán file thật (CF_HDROP), không chỉ dán đường dẫn.
- `Enter` **auto-paste** thẳng vào cửa sổ trước · `⇧Enter` dán plain text · `Ctrl+1..9` dán nhanh
- `Ctrl+P` ghim (item ghim không bị xóa tự động) · `Ctrl+S` biến thành Snippet · `Del` xóa
- Sửa nội dung là **tự lưu**. **Privacy Guard**: tự bỏ qua clipboard từ KeePass/Bitwarden/1Password…
- Tự dọn: giữ tối đa N item chưa ghim / theo số ngày (chỉnh trong Settings), tự xóa cache ảnh mồ côi.

## Cửa sổ Settings

Cửa sổ Windows riêng (có viền, taskbar). Cho phép chỉnh: hotkey mở app, **keyword của mọi tính năng**, theme (system/light/dark), auto-paste, giới hạn/thời gian lưu clipboard, Privacy Guard, tự khởi động cùng Windows, và bật/tắt các tính năng nhạy cảm.

## Tính năng nhạy cảm (mặc định TẮT)

- **Mật khẩu trình duyệt** (`pw`): đọc mật khẩu đã lưu trong Edge/Chrome/Brave/Cốc Cốc… của **chính tài khoản Windows đang đăng nhập** (giải mã DPAPI + AES-GCM, giống tính năng Export passwords của trình duyệt). Chỉ hoạt động cục bộ. Khi copy, mật khẩu **không lưu vào clipboard history** (có toggle riêng để đổi).
- **Kill tiến trình mạnh** (`ps` → Kill): bật SeDebugPrivilege + TerminateProcess, fallback `taskkill /F /T` quyền admin — mạnh hơn Task Manager thường.

## Kiến trúc

- **Frontend** (`src/`): React + TypeScript + Tailwind + Framer Motion.
- **Backend** (`src-tauri/`): Rust — hotkey, index app (Start Menu + Registry), clipboard watcher, SQLite (`%APPDATA%\heaspot\heaspot.db`), icon thật (Shell API), tray.
- **File search**: **Everything bundle sẵn** trong app, tự chạy nền **ẩn (không tray)**; fallback index nội bộ nếu cần. Không cần cài Everything riêng.
- **Full-text**: Windows Search (`Search.CollatorDSO`) + Capacities API. Độc lập với Everything.

## Phát triển

```bash
npm install
npm run tauri dev
```

Yêu cầu: Node.js, Rust (MSVC), VS Build Tools (C++), WebView2.

## Đóng gói .exe

```bash
npm run tauri build
```

Bộ cài NSIS ở `src-tauri/target/release/bundle/nsis/HeaSpot_*_x64-setup.exe`.

> Khi build: nếu Everything đang chạy và khóa file, tắt nó trước. HeaSpot dùng bản Everything ở `src-tauri/resources/heaspot-everything/` khi đóng gói.

## CI — tự động release

`.github/workflows/release.yml`:
- Đẩy tag `v*` (VD `git tag v0.1.3 && git push --tags`) → tạo **release chính thức** kèm bộ cài.
- Push lên `main` → cập nhật bản **nightly** (prerelease) build mới nhất.
