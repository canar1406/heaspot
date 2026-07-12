# HeaSpot

**HeaSpot** — launcher kiểu **Spotlight (macOS) / Alfred** cho Windows, xây bằng **Tauri (Rust + React)**. Siêu nhẹ (~10 MB RAM khi ẩn), keyboard-first, mở tức thì.

## Phím tắt toàn cục

| Phím | Chức năng |
|---|---|
| `Alt + Space` | Mở / ẩn thanh tìm kiếm |
| `Win + V` | Mở Clipboard Manager kiểu Alfred |

> `Win+V` được bắt bằng low-level keyboard hook nên luôn thắng panel clipboard mặc định của Windows. Nếu `Alt+Space` bị PowerToys Run… chiếm, HeaSpot tự chuyển sang hook để vẫn hoạt động. **Chỉ đúng một hotkey clipboard bạn đặt trong Settings hoạt động** — không còn phím dự phòng ẩn nào khác.

![Thanh tìm kiếm HeaSpot](docs/screenshots/search-launcher.png)

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
| `conv 60 km/h to m/s` / `conv 255 dec to hex` | **Đổi đơn vị mọi nhóm bản chất** + cơ số / ASCII |
| `time tokyo` / `time gmt+7` | Giờ thế giới |
| `g <từ khoá>` · `yt <từ khoá>` | **Kết quả nhanh Google** (preview) + tìm chi tiết · YouTube |
| `url github.com` | Mở URL |
| `#uuid` · `#md5 text` · `#b64 text` | Tạo UUID / hash / base64 |
| `ps <tên>` | **Task Manager** — gom process trùng tên; Enter/→ bung các PID con |
| `port 3000` | **Dev**: tìm & Kill tiến trình đang chiếm cổng TCP |
| `json` | **Dev**: format/minify/kiểm tra JSON trong clipboard |
| `jwt <token>` | **Dev**: giải mã JWT (header/payload, exp) |
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

Điều hướng: `↑ ↓` chọn · `Enter` mở · **`→` mở Context Menu** (Run as admin, Open location, Copy path, Kill…) · `Tab` điền · `Ctrl+1..9` mở nhanh · `Esc` đóng. Selection được giữ ổn định khi kết quả async cập nhật và chỉ trở về item đầu khi người dùng sửa truy vấn.

![Tìm & mở app với icon thật](docs/screenshots/app-search.png)

![Context Menu điều hướng hoàn toàn bằng bàn phím](docs/screenshots/context-menu.png)

### Full-text trong tài liệu (`in`)

`in <từ khoá>` chỉ tìm **nội dung** trong các định dạng tài liệu/code được hỗ trợ ở thư mục người dùng; file thực thi và kho thành phần Windows (`.exe`, `.dll`, WinSxS, WindowsApps…) bị loại khỏi kết quả. Có thể kết nối Capacities để tìm đồng thời trong note.

![Tìm nội dung tài liệu và kết nối Capacities](docs/screenshots/fulltext.png)

## Đổi đơn vị & cơ số (`conv`)

Đổi **mọi đơn vị cùng bản chất**, tính toán cục bộ nên tức thì. Các đơn vị được chia theo **nhóm bản chất** — chỉ đổi được trong cùng nhóm, đổi chéo giữa hai nhóm khác nhau sẽ báo lỗi rõ ràng thay vì âm thầm cho kết quả sai:

| Nhóm | Đơn vị hỗ trợ (một số) |
|---|---|
| Chiều dài | m, km, cm, mm, µm, nm, mi, ft, in, yd, nmi, ly |
| Khối lượng | kg, g, mg, µg, t (tấn), lb, oz, st, ct |
| Thời gian | s, ms, µs, ns, min, h, day, week, month, year |
| Dữ liệu | b, kb, mb, gb, tb, pb, bit, kbit, mbit, gbit (nhị phân 1024) |
| Nhiệt độ | °C, °F, K |
| Diện tích | m², km², cm², ha, are, acre, ft², in², mi², yd² |
| Thể tích | l, ml, m³, cm³, gal, qt, pt, cup, floz, tbsp, tsp, ft³, in³ |
| Tốc độ | m/s, km/h, mph, ft/s, knot |
| Áp suất | pa, kpa, hpa, bar, atm, psi, mmhg, torr, inhg |
| Năng lượng | j, kj, cal, kcal, wh, kwh, ev, btu |
| Công suất | w, kw, mw, hp, ps |
| Góc | deg, rad, grad, arcmin, arcsec, turn |
| Tần số | hz, khz, mhz, ghz, thz, rpm |
| Cơ số | bin ⇆ dec ⇆ hex ⇆ ascii (`conv 255 dec to hex`, `conv ff hex to bin`) |

Nối bằng `to` / `sang` / `ra` / `thành` / `=` / `->`. Ví dụ: `conv 60 km/h to m/s`, `conv 1 atm sang psi`, `conv 100 f to c`.

![Đổi đơn vị nhiều nhóm bản chất](docs/screenshots/converter.png)

## Máy tính và thời gian

Máy tính nhận biểu thức sau `=` (hằng số, lượng giác, lũy thừa); `time <thành phố/GMT>` đổi giờ thế giới. Kết quả được tính cục bộ và `Enter` copy nhanh.

![Máy tính nâng cao](docs/screenshots/calculator.png)

![Giờ thế giới](docs/screenshots/timezones.png)

## Dịch, Wikipedia và công thức học tập

- `tr` hợp nhất dịch và từ điển: bản dịch hai chiều, IPA/audio, word forms, nghĩa Anh–Việt, ví dụ, collocation, đồng/trái nghĩa và lưu từ ôn tập.
- `wiki` tải tiêu đề nhanh trước rồi bổ sung phần mở đầu chi tiết trong preview.
- `formula` tra catalog Toán/Lý/Hóa offline; nếu chưa có thì fallback Wikipedia.

![Smart Translate và từ điển học thuật](docs/screenshots/translate.png)

![Wikipedia preview](docs/screenshots/wikipedia.png)

![Tra công thức Toán Lý Hóa](docs/screenshots/formula.png)

## Kết quả nhanh Google (`g`)

Gõ `g <từ khoá>`: HeaSpot lấy **kết quả nhanh của Google** (answer box / knowledge graph / snippet) và hiển thị ngay ở **khung preview bên phải**; bên trái có thêm dòng **"Tìm chi tiết với Google"** để mở trang tìm kiếm đầy đủ. Ưu tiên tốc độ: kết quả nhanh hiện trước, chi tiết bổ sung sau.

Nguồn dữ liệu dùng **Serper.dev** (gói miễn phí 2.500 lượt, không cần thẻ). Dán API key vào **Settings → Chung → Kết quả nhanh Google (g)** rồi Lưu; nếu để trống, HeaSpot tự lùi về DuckDuckGo Instant Answer. API key chỉ lưu cục bộ trong SQLite, không gửi đi đâu khác.

![Kết quả nhanh Google](docs/screenshots/google-quick.png)

## Clipboard Manager (Win+V)

Giao diện 2 cột kiểu Alfred: danh sách bên trái, **preview chỉnh sửa trực tiếp** bên phải.

- Lưu **text, link, ảnh (thumbnail + preview), danh sách file** — dán file thật (CF_HDROP), không chỉ dán đường dẫn.
- `Enter` **auto-paste** thẳng vào cửa sổ trước · `⇧Enter` dán plain text · `Ctrl+1..9` dán nhanh
- `Ctrl+P` ghim (item ghim không bị xóa tự động) · `Ctrl+S` biến thành Snippet · `Del` xóa
- Mỗi hàng bên trái có icon **Ghim** và **Xóa** để click trực tiếp mà không làm mất focus bàn phím.
- Item vừa ghim được đẩy ngay lên đầu; bỏ ghim trả về thứ tự thời gian.
- Sửa nội dung là **tự lưu**. **Privacy Guard**: tự bỏ qua clipboard từ KeePass/Bitwarden/1Password…
- Tự dọn: giữ tối đa N item chưa ghim / theo số ngày (chỉnh trong Settings), tự xóa cache ảnh mồ côi.
- **Zoom ảnh trong preview**: lăn chuột để phóng to/thu nhỏ về phía con trỏ (1–10×), kéo để di chuyển, nút `+ / − / %`, double-click để reset. Áp dụng cho mọi khung xem ảnh.

![Clipboard Manager 2 cột](docs/screenshots/clipboard.png)

![Zoom ảnh trong preview](docs/screenshots/image-zoom.png)

## Snippets (`;`) — gõ tắt

**Snippet** là một đoạn văn bản mẫu **được lưu cố định** dưới một từ khoá ngắn, để dùng lại nhiều lần mà không phải gõ tay hay đi tìm để copy. Ví dụ lưu email, địa chỉ, số tài khoản, mẫu trả lời, chữ ký… dưới các keyword như `mail`, `stk`, `sign`.

Khác với clipboard history (lưu những gì bạn *vừa* copy và sẽ tự xoá theo thời gian), snippet **không bị tự xoá** — nó là thư viện văn bản riêng của bạn.

- **Tạo**: gõ `snip <keyword> <nội dung>` (VD `snip mail longvo@gmail.com`), hoặc trong Clipboard nhấn `Ctrl+S` để biến item đang chọn thành snippet.
- **Dùng**: gõ `;` để xem tất cả snippet, hoặc `;<keyword>` để lọc nhanh; `Enter` dán/copy nội dung.

![Snippets gõ tắt](docs/screenshots/snippets.png)

## Cửa sổ Settings

Cửa sổ Windows riêng (có viền, taskbar). Cho phép chỉnh: hotkey mở app, **keyword và global hotkey riêng của mọi tính năng**, theme (system/light/dark), auto-paste, giới hạn/thời gian lưu clipboard, Privacy Guard, tự khởi động cùng Windows, API key Serper cho `g`, và bật/tắt các tính năng nhạy cảm.

![Cửa sổ Settings](docs/screenshots/settings.png)

### Dọn cache toàn app (Clear cache)

Trong tab **Chung** có nút **🧹 Dọn toàn bộ cache ngay**: xoá **mọi cache tra cứu của tất cả tính năng** trong RAM (dịch, tra nhanh, Wikipedia, kết quả Google, full-text Windows Search, Capacities, icon app/file) và các ảnh clip tạm mồ côi trên đĩa, rồi báo dung lượng đã giải phóng. Dành cho người dùng lâu ngày muốn dọn dẹp — **không** đụng tới clipboard history, snippet hay cấu hình đã lưu.

![Nút Dọn cache trong Settings](docs/screenshots/clear-cache.png)

### Feature Hotkeys và text đang bôi đen

Trong tab **Keyword & Hotkey tính năng**, click ô Hotkey rồi nhấn tổ hợp mong muốn, ví dụ `Alt+Shift+T` cho Translate. Khi đang bôi đen nội dung ở trình duyệt/editor:

- Hotkey Translate mở `tr <selection>`.
- Hotkey Converter mở `conv <selection>`.
- Hotkey Formula/Wikipedia/Full-text mở feature tương ứng với selection.
- Nếu không có selection, HeaSpot mở sẵn keyword để nhập tiếp.
- HeaSpot copy selection tạm thời, khóa clipboard watcher và khôi phục clipboard text/ảnh cũ trước khi mở cửa sổ; không tự Enter/chạy hành động nguy hiểm.

![Cấu hình keyword và global hotkey riêng cho từng tính năng](docs/screenshots/feature-hotkeys.png)

### OCR tiếng Việt

Gõ `ocr` hoặc gán hotkey riêng để chụp một vùng màn hình. HeaSpot tiền xử lý ảnh (phóng to chữ nhỏ, tăng tương phản và làm nét), sau đó ưu tiên **Tesseract 5 với model `vie+eng`**; không fallback âm thầm sang model tiếng Anh vì sẽ làm sai dấu. OCR chạy offline và không giữ ảnh tạm sau khi nhận dạng.

Khung **preview bên phải cho chỉnh sửa trực tiếp**: sửa văn bản OCR ngay tại chỗ, **tự lưu**, và nhấn `Enter` để **tự copy** kết quả đã sửa vào clipboard.

![OCR tiếng Việt — preview chỉnh sửa trực tiếp](docs/screenshots/ocr.png)

Bản phát hành đã **nhúng sẵn Tesseract 5 và hai model `vie+eng`** trong bundle, nên người dùng không phải cài language pack hay tải cache sau khi cài app. Bản Tesseract hệ thống/đường dẫn `HEASPOT_TESSERACT_PATH` chỉ là fallback cho môi trường development; Windows Media OCR chỉ được dùng khi hệ thống thực sự có recognizer `vi-*`.

## Tính năng nhạy cảm (mặc định TẮT)

- **Mật khẩu trình duyệt** (`pw`): đọc mật khẩu đã lưu trong Edge/Chrome/Brave/Cốc Cốc… của **chính tài khoản Windows đang đăng nhập** (giải mã DPAPI + AES-GCM, giống tính năng Export passwords của trình duyệt). Chỉ hoạt động cục bộ. Khi copy, mật khẩu **không lưu vào clipboard history** (có toggle riêng để đổi).
- **Task Manager dạng nhóm**: process trùng tên được gom thành một hàng tổng RAM/số instance; Enter hoặc → bung PID con, ← thu nhóm.
- **Kill tiến trình mạnh** (`ps` → bung nhóm → chọn PID → Kill): bật SeDebugPrivilege + TerminateProcess, fallback `taskkill /F /T` quyền admin.
- Search và detail preview giữ cùng chiều rộng; resize chiều cao có animation hủy frame cũ để tránh giật khi chuyển chế độ.

![Task Manager gom tiến trình trùng tên](docs/screenshots/task-manager.png)

## Kiến trúc

- **Frontend** (`src/`): React + TypeScript + Tailwind + Framer Motion.
- **Backend** (`src-tauri/`): Rust — hotkey, index app (Start Menu + Registry + UWP/Store qua `Get-StartApps`), clipboard watcher, SQLite (`%APPDATA%\heaspot\heaspot.db`), icon thật (Shell/AppxManifest), tray.
- **File search**: **Everything bundle sẵn** trong app, tự chạy nền **ẩn (không tray)**; fallback index nội bộ nếu cần. Kho package hệ thống như WinSxS/WindowsApps/AppRepository được lọc khỏi launcher. Không cần cài Everything riêng.
- **Full-text**: Windows Search (`Search.CollatorDSO`) + Capacities API; chỉ nhận nhóm tài liệu/code, loại file thực thi và độc lập với Everything.

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
