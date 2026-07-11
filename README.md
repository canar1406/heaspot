# WinSpot

WinSpot là launcher kiểu Spotlight/Alfred dành cho Windows, xây dựng bằng **Tauri 2, Rust, React và TypeScript**. Ứng dụng tập trung vào tốc độ, thao tác bàn phím và tích hợp trực tiếp với các chức năng của Windows.

## Tính năng chính

- Tìm và mở app, file, folder bằng fuzzy search.
- Tìm file toàn máy bằng Everything SDK được bundle sẵn.
- Tìm nội dung Word, PDF, Excel và tài liệu khác qua Windows Search Index.
- Clipboard Manager có lịch sử text, link, ảnh và file.
- Paste file thật bằng chuẩn Windows `CF_HDROP`.
- Calculator, Unit Converter và chuyển đổi Binary/Decimal/Hex/ASCII.
- Tra Wikipedia có panel preview chi tiết.
- Smart Translate có phát âm, word forms và định nghĩa Anh–Việt.
- Window Walker, VS Code Recent, Windows Services và Registry Browser.
- Snippets/Text Expander và Alfred Script Filter workflows.
- Context menu cho app/file/folder/service, bao gồm gỡ cài đặt app.
- Panel Settings kiểu Alfred để cấu hình hotkey, clipboard, privacy và startup.

## Phím tắt toàn cục

| Phím mặc định | Chức năng |
|---|---|
| `Alt + Space` | Mở hoặc ẩn launcher |
| `Win + V` | Mở Clipboard Manager và chặn panel clipboard mặc định của Windows |
| `Ctrl + Shift + V` | Phím dự phòng mở Clipboard Manager |

Hotkey có thể thay đổi trong `settings`:

- Launcher: `Alt+Space`, `Ctrl+Space`, `Ctrl+Alt+Space`, `Alt+F1`.
- Clipboard: `Win+V`, `Ctrl+Shift+V`, `Alt+V`, `Ctrl+Alt+V`.
- Nếu `Alt+Space` bị ứng dụng khác chiếm, WinSpot có cơ chế low-level keyboard hook dự phòng.

## Cú pháp tìm kiếm

### App, file và nội dung tài liệu

| Cú pháp | Chức năng |
|---|---|
| `chrome` | Tìm app, file và folder |
| `in hợp đồng` | Tìm nội dung bên trong tài liệu qua Windows Search và Capacities |
| `doc:báo cáo` | Alias của full-text search |
| `capacities token <token>` | Lưu Capacities API token |

Kết quả app được ưu tiên hơn file. WinSpot quét Start Menu, Registry App Paths và trích xuất icon thật bằng Windows Shell API.

### Calculator

Hỗ trợ `+`, `-`, `*`, `/`, `^`, `%`, ngoặc, số âm và các hàm:

- `sin`, `cos`, `tan`, `asin`, `acos`, `atan`.
- `sqrt`, `abs`, `log`, `ln`, `log2`, `exp`.
- `round`, `floor`, `ceil`.
- Hằng số `pi`, `e`, `tau`.

Ví dụ:

```text
100 * 20%
=sin(pi/2)
100*(50+20)/2
```

Enter để copy kết quả.

### Unit Converter và Data Converter

Unit Converter hỗ trợ chiều dài, khối lượng, thời gian, dung lượng dữ liệu và nhiệt độ:

```text
10 ft to m
5 kg sang lb
100 f to c
2 gb to mb
```

Chuyển đổi Binary/Decimal/Hex/ASCII:

```text
255 dec to hex
0xff to bin
1010 bin to dec
hex 48 69 to ascii
ascii Hello to hex
ascii Hello to bin
```

Số nguyên lớn được xử lý bằng `BigInt`; chuỗi ASCII được chuyển theo byte UTF-8.

### Wikipedia và từ điển nhanh

```text
wiki trí tuệ nhân tạo
dict machine learning
```

- Hiển thị tối đa 5 kết quả từ Wikipedia tiếng Việt.
- Panel bên phải hiển thị phần mở đầu đầy đủ.
- `Enter` hoặc nút **Dán** để paste nội dung vào ứng dụng trước đó.
- Có nút **Copy** và **Mở nguồn**.

### Smart Translate

```text
tr hello
translate artificial intelligence
tr xin chào
```

- Tự nhận diện ngôn ngữ.
- Tiếng Việt được dịch sang tiếng Anh; ngôn ngữ khác mặc định dịch sang tiếng Việt.
- Với một từ tiếng Anh, preview bổ sung:
  - Phát âm/phonetic.
  - Loại từ và word forms.
  - Định nghĩa tiếng Anh.
  - Nghĩa tiếng Việt.
  - Ví dụ sử dụng nếu nguồn có cung cấp.
- Enter dán bản dịch chính; panel preview giữ phần thông tin chi tiết.

Smart Translate hiện sử dụng Google Translate public endpoint và Dictionary API công cộng. Khi triển khai cho lượng người dùng lớn nên bổ sung provider/API key dự phòng để tránh rate limit.

### Web Search và URL

| Cú pháp | Chức năng |
|---|---|
| `g từ khoá` | Google Search |
| `yt từ khoá` | YouTube Search |
| `github.com` | Mở domain bằng trình duyệt mặc định |
| `a@example.com` | Mở email bằng `mailto:` |

### Công cụ Windows

| Cú pháp | Chức năng |
|---|---|
| `> ping google.com` | Mở CMD mới, chạy lệnh và giữ terminal mở |
| `< edge` | Window Walker: tìm và focus cửa sổ đang mở |
| `{ winspot` | Mở project/workspace gần đây của VS Code |
| `! print` | Tìm Windows Services |
| `: hkcu\software` | Duyệt Registry và mở Regedit đúng key |
| `# uuid` | Tạo UUID v4 |
| `# md5 text` | MD5 |
| `# sha1 text` | SHA-1 |
| `# sha256 text` | SHA-256 |
| `# b64 text` | Base64 encode |
| `# b64d text` | Base64 decode |
| `time tokyo` | Giờ và ngày theo timezone |

Các lệnh hệ thống được nhận diện trực tiếp: `sleep`, `shutdown`, `restart`, `lock`, `mute`, `empty trash`.

### Snippets

```text
;
;mail
snip mail hello@example.com
```

- `;` liệt kê toàn bộ snippets.
- Tìm theo keyword hoặc nội dung.
- Keyword trùng sẽ cập nhật nội dung cũ.
- Clipboard text/link có thể chuyển thành snippet bằng `Ctrl+S`.

### Alfred Workflow Compatibility

```text
workflow
workflow rescan
workflow install C:\path\example.alfredworkflow
```

WinSpot hỗ trợ Alfred Script Filter trả về JSON và các runner:

- Python.
- Node.js.
- PHP.
- Ruby.
- Perl.
- Bash/Git Bash.

Runtime tương ứng phải có trong `PATH`. AppleScript/JXA và Alfred XML output cũ chưa được hỗ trợ. Workflow được giải nén vào `%APPDATA%\winspot\workflows`.

## Context Menu

Chọn kết quả rồi nhấn `→` để mở context menu.

App:

- Mở.
- Run as administrator.
- Open file location.
- Copy path.
- Run in Terminal.
- Gỡ cài đặt.

File/folder:

- Mở.
- Open file location.
- Copy path.
- Mở Terminal tại folder.

Windows Services:

- Start.
- Stop.
- Restart.
- Copy service name.

### Gỡ cài đặt app

WinSpot tìm `UninstallString` trong các Registry key mà Control Panel/Programs and Features sử dụng. Với MSI, lệnh maintenance `/I` được chuyển thành uninstall `/X`. Nếu không ghép được app với package chính xác, WinSpot mở `appwiz.cpl` để người dùng chọn thủ công thay vì xóa nhầm dữ liệu.

## Clipboard Manager

Mở bằng `Win+V` hoặc hotkey đã chọn trong Settings.

### Dữ liệu được lưu

- Text.
- Link.
- Ảnh clipboard/screenshot dưới dạng PNG.
- Danh sách file copy từ Explorer.
- Source process và thời điểm copy.

Ảnh được lưu trong `%APPDATA%\winspot\clips`; metadata nằm trong SQLite.

### Điều khiển

| Phím | Chức năng |
|---|---|
| `↑` / `↓` | Chọn item |
| `Enter` | Paste item vào ứng dụng trước đó |
| `Shift + Enter` | Plain paste; với ảnh/file sẽ dán đường dẫn |
| `Ctrl + 1..9` | Paste nhanh item 1–9 |
| `Ctrl + P` | Ghim/bỏ ghim |
| `Ctrl + S` | Lưu text/link thành snippet |
| `Tab` / `Ctrl + E` | Focus khung chỉnh sửa |
| `Ctrl + Enter` | Paste nội dung đang chỉnh sửa |
| `Delete` / `Ctrl + D` | Xóa item |
| `Ctrl + L` | Xóa toàn bộ item chưa ghim |
| `Esc` | Quay lại ô tìm kiếm hoặc đóng |

Text và link trong preview tự lưu sau 600ms.

### Paste file thật

File được ghi lại vào clipboard bằng `CF_HDROP`, vì vậy Explorer và ứng dụng đích nhận đúng file thật thay vì một chuỗi đường dẫn. `Shift+Enter` vẫn cho phép dán danh sách đường dẫn dưới dạng text.

### Privacy và chống rác

- Tôn trọng format Windows `ExcludeClipboardContentFromMonitorProcessing`.
- Mặc định bỏ qua KeePass, Bitwarden, 1Password, LastPass, Dashlane và Proton Pass.
- Danh sách process riêng tư có thể chỉnh trong Settings.
- Giới hạn từ 50 đến 2000 item chưa ghim.
- Thời gian lưu có thể chọn:
  - Giữ mãi.
  - 1 ngày.
  - 7 ngày.
  - 30 ngày.
  - 90 ngày.
  - 1 năm.
- Item đã ghim không bị auto-cleanup hoặc Clear All xóa.
- Khi record ảnh hết hạn hoặc bị dọn, file PNG tương ứng cũng được xóa.
- PNG mồ côi do crash/nâng cấp được dọn khi WinSpot khởi động.

## Settings

Mở bằng:

```text
settings
```

Hoặc chọn **Cài đặt…** từ system tray.

Panel Settings gồm:

### General

- Chọn hotkey launcher.
- Chọn hotkey Clipboard Manager.
- Tự khởi động WinSpot cùng Windows qua `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

### Clipboard & Privacy

- Chọn số ngày lưu lịch sử.
- Chọn số lượng item chưa ghim tối đa.
- Cấu hình danh sách process không được ghi clipboard.

### Cú pháp & tính năng

- Hiển thị danh sách cú pháp nhanh ngay trong Settings.

Cấu hình được lưu trong bảng `settings` của `%APPDATA%\winspot\winspot.db`.

## Điều hướng bàn phím

| Phím | Chức năng |
|---|---|
| `↑` / `↓` | Chọn kết quả |
| `Enter` | Thực thi kết quả |
| `→` | Mở context menu |
| `←` | Đóng context menu |
| `Tab` | Điền tên app/file/folder vào ô tìm kiếm |
| `Ctrl + 1..9` | Thực thi nhanh kết quả 1–9 |
| `Esc` | Xóa query hoặc đóng WinSpot |

## Kiến trúc

- **Frontend:** React 18, TypeScript, Tailwind CSS, Framer Motion.
- **Desktop:** Tauri 2.
- **Backend:** Rust và Windows API.
- **Database:** SQLite WAL tại `%APPDATA%\winspot\winspot.db`.
- **File search:** Everything SDK qua `Everything64.dll`.
- **Fallback index:** Desktop, Documents, Downloads, Pictures, Videos và Music; tối đa 60.000 mục, refresh mỗi 10 phút.
- **Full-text:** Windows Search `Search.CollatorDSO`.
- **Clipboard:** `arboard`, SQLite, PNG cache và Windows `CF_HDROP`.
- **Installer:** NSIS current-user.

## Phát triển

Yêu cầu:

- Node.js.
- Rust MSVC toolchain.
- Visual Studio Build Tools C++.
- WebView2.

```bash
npm install
npm run tauri dev
```

Build frontend:

```bash
npm run build
```

Kiểm tra Rust:

```bash
cd src-tauri
cargo check
```

## Đóng gói

```bash
npm run tauri build
```

Kết quả:

- Binary: `src-tauri/target/release/winspot.exe`.
- Installer: `src-tauri/target/release/bundle/nsis/WinSpot_0.1.0_x64-setup.exe`.

## Giới hạn hiện tại

- Chỉ hỗ trợ Windows.
- Clipboard chưa lưu RTF/rich text.
- Registry plugin duyệt key nhưng chưa hiển thị value.
- Currency Converter chưa được tích hợp.
- Alfred Workflow mới hỗ trợ Script Filter đầu tiên và JSON output.
- Smart Translate phụ thuộc dịch vụ public và có thể bị rate limit.
- Capacities token hiện lưu trong SQLite, chưa dùng Windows Credential Manager.
