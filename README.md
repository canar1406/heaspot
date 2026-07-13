# HeaSpot

**HeaSpot** — launcher kiểu **Spotlight (macOS) / Alfred** cho Windows, xây bằng **Tauri (Rust + React)**. Siêu nhẹ (~10 MB RAM khi ẩn), keyboard-first, mở tức thì.

### Mới trong v0.1.8

- `in` dùng chiến lược hybrid: Windows Search nhanh trước, tự fallback Everything `content:` khi file nằm ngoài index; mỗi kết quả hiện đúng engine thực tế.
- Sửa instance Everything bundled có thể chạy với database rỗng: portable Folder Index theo user home, monitor thay đổi, không cần service hay UAC.
- Khóa trạng thái Everything SDK để search tên file và content search không ghi đè nhau; lọc cây cache/build để fallback nhanh hơn.
- Thêm trạng thái loading cho full-text, cache kết quả và bộ smoke test có fixture thật.
- README được mở rộng toàn bộ kiến trúc/kỹ thuật và ảnh tài liệu được chụp lại bằng profile cô lập, nền sạch.

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
| `in <từ khoá>` / `doc:<từ khoá>` | Tìm **nội dung trong file** bằng Windows Search, tự fallback Everything `content:` **+ note Capacities** |
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

`in <từ khoá>` chỉ tìm **nội dung** trong các định dạng tài liệu/code được hỗ trợ ở thư mục người dùng. HeaSpot ưu tiên **Windows Search** vì index đảo cho kết quả nhanh, có xếp hạng và đoạn trích; nếu không tìm thấy, app tự fallback sang **Everything 1.4 `content:`** để quét cả file nằm ngoài vùng Windows đã index. Kết quả ghi rõ engine thực tế là `Windows Search` hay `Everything`. File thực thi và kho thành phần Windows (`.exe`, `.dll`, WinSxS, WindowsApps…) bị loại khỏi kết quả. Có thể kết nối Capacities để tìm đồng thời trong note.

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

Trong tab **Chung** có nút **🧹 Dọn toàn bộ cache ngay**: xoá **mọi cache tra cứu của tất cả tính năng** trong RAM (dịch, tra nhanh, Wikipedia, kết quả Google, full-text hybrid, Capacities, icon app/file) và các ảnh clip tạm mồ côi trên đĩa, rồi báo dung lượng đã giải phóng. Dành cho người dùng lâu ngày muốn dọn dẹp — **không** đụng tới clipboard history, snippet hay cấu hình đã lưu.

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

### Tổng quan luồng dữ liệu

```mermaid
flowchart LR
    A["Global hotkey / Win+V / Tray"] --> B["Tauri window controller"]
    B --> C["React UI: Search / Clipboard / Settings"]
    C --> D["useSearch + keyword router"]
    D --> E["Plugin TypeScript cục bộ"]
    D --> F["Tauri IPC commands"]
    F --> G["Everything / Windows Search / Windows API"]
    F --> H["SQLite + cache + clipboard"]
    F --> I["HTTP API tùy chọn"]
    E --> J["Result list + detail preview"]
    G --> J
    H --> J
    I --> J
    J --> K["Open / paste / copy / system action"]
```

App là kiến trúc **desktop hai lớp**: WebView chỉ phụ trách giao diện và plugin tính toán an toàn; Rust giữ toàn bộ thao tác hệ thống, dữ liệu cục bộ và tài nguyên nhúng. Hai lớp trao đổi bằng Tauri IPC có kiểu dữ liệu Serde/TypeScript, không mở HTTP server cục bộ.

### Frontend (`src/`)

- **React 18 + TypeScript + Vite**: `App.tsx` điều phối ba mode Search/Clipboard/Settings; `main.tsx` dùng cùng bundle nhưng render `SettingsView` cho cửa sổ `settings` riêng.
- **Tailwind CSS + CSS toàn cục**: theme sáng/tối/system, layout hai cột, trạng thái selected/focus và các icon action.
- **Framer Motion**: animation mở launcher bằng opacity/scale/translate; không remount cây UI. Native window chỉ resize khi kích thước thật sự đổi để tránh khựng khi gõ/chuyển mode.
- **Router theo keyword trong `useSearch.ts`**: chỉ kích hoạt tính năng khi token đầu khớp đúng keyword (`in`, `tr`, `conv`, `ps`…), debounce request async và dùng sequence guard để kết quả cũ không ghi đè truy vấn mới.
- **Plugin chạy cục bộ trong `src/plugins/`**: calculator, converter/cơ số, timezone, công thức, hoá học, JSON/JWT, LaTeX, generator, URL/web/system command parser. Các phép tính này không gọi mạng.
- **Điều hướng bàn phím**: `useKeyboardNav` giữ selection hợp lệ khi list async thay đổi; Context Menu, nhóm process và Clipboard đều dùng chung quy ước ↑/↓/←/→/Enter/Esc.
- **Preview theo loại dữ liệu**: `KnowledgePreview` hiển thị dịch/từ điển/wiki/formula/OCR; `ZoomableImage` xử lý zoom theo con trỏ và pan ảnh; Clipboard preview hỗ trợ sửa tự lưu.

### Backend Tauri/Rust (`src-tauri/`)

- **Tauri 2** tạo launcher trong suốt, cửa sổ Settings riêng và system tray. `lib.rs` đăng ký state dùng chung (`RwLock` cho index app/file, `Mutex` cho SQLite) và toàn bộ IPC command.
- **`core/window.rs`**: show/hide/focus, resize native, giữ foreground window để auto-paste, trim working set khi ẩn và context menu gỡ cài đặt/run admin/open location.
- **`core/hotkey.rs`**: global shortcut mặc định và hotkey riêng từng feature; khi gọi trên đoạn đang bôi đen, app tạm copy selection, chặn clipboard watcher, khôi phục clipboard cũ rồi prefill keyword.
- **`core/indexer.rs` + `core/icons.rs`**: quét Start Menu, Registry App Paths và UWP/Store qua `Get-StartApps`; mở UWP bằng `shell:AppsFolder`; trích icon Shell hoặc asset trong AppxManifest và cache dưới dạng PNG data URL.
- **`commands/`**: search/full-text, clipboard, settings, snippets, OCR, translate/wiki/Google quick answer, Capacities, study words và Windows system actions.
- **`plugins/` phía Rust**: process/task manager, services, registry, VS Code recent projects, window walker, browser passwords, Alfred Workflow và generator/hash.
- **Windows API qua `windows-sys`**: foreground/focus, phím giả lập, GDI/capture, Shell, COM, clipboard/CF_HDROP, process/token privilege, DPAPI, power/shutdown và memory trimming. PowerShell ẩn chỉ được dùng cho các bề mặt Windows phù hợp như UWP discovery, Windows Search OLE DB và một số system query.

### Các engine tìm kiếm

| Phạm vi | Engine chính | Fallback / kỹ thuật |
|---|---|---|
| App | Index nền Start Menu + Registry + UWP | Fuzzy score: exact → prefix → word boundary → substring → subsequence |
| Tên file/thư mục | Everything SDK qua `Everything64.dll` | Index nội bộ các thư mục người dùng nếu IPC Everything chưa sẵn sàng |
| Nội dung file (`in`) | Windows Search `Search.CollatorDSO` | Khi rỗng, Everything `content:` qua iFilter/UTF-8 |
| Note | Capacities API | Chạy song song với full-text khi đã cấu hình token |
| Wiki/dịch/Google | Wikipedia, Google Translate/Dictionary, Serper | Cache RAM; Google quick answer fallback DuckDuckGo khi chưa có key |

Everything 1.4.1 được nhúng và chạy nền ẩn. Để không đòi UAC hay cài Everything Service, instance portable của HeaSpot dùng **Folder Index + change monitor cho thư mục user home**; nếu máy đã có instance Everything hoạt động thì SDK dùng trực tiếp instance đó. SDK giữ trạng thái truy vấn cấp process nên backend đặt `Mutex` quanh mọi query, tránh search tên file và fallback `content:` ghi đè lẫn nhau. Với `in`, `content:` luôn đặt **sau** bộ lọc `file:`, đường dẫn home, danh sách extension và loại AppData/node_modules/.git/target/dist để giảm lượng file phải mở. Kho hệ thống WinSxS/WindowsApps/AppRepository, Recycle Bin và file `.exe/.dll` bị lọc khỏi kết quả.

### Dữ liệu, cache và vòng đời

- **SQLite bundled (`rusqlite`)** tại `%APPDATA%\heaspot\heaspot.db`, bật **WAL** để watcher ghi trong khi UI đọc. Bảng chính: `clipboard`, `snippets`, `settings`, `study_words`; migration cột chạy idempotent lúc mở DB.
- **Clipboard watcher** poll cục bộ, nhận text/link/image/file list (CF_HDROP), deduplicate, phát event realtime và auto-prune theo số item/số ngày. Item ghim được bảo toàn; ảnh PNG/thumbnail nằm trong thư mục `clips` cạnh DB.
- **Cache RAM có giới hạn** cho icon, translate/wiki/quick answer/full-text/Capacities; nút Clear cache xoá cache lookup và ảnh mồ côi nhưng không xoá dữ liệu người dùng.
- **Everything/Tesseract là tài nguyên nhúng**. Tesseract 5 dùng model `vie+eng`; script build kiểm SHA-256 và chỉ đóng gói runtime/model cần thiết để không phình bộ cài hoặc sinh cache tải về.
- Biến `HEASPOT_DATA_DIR` chỉ dành cho smoke test/tài liệu: chuyển DB và cache sang profile tạm, bảo đảm quá trình chụp ảnh không đọc clipboard/settings thật.

### Bảo mật và riêng tư

- Mặc định các tính năng nhạy cảm như browser password bị tắt. Secret giải mã bằng **Windows DPAPI + AES-GCM** trong đúng user session, không gửi mạng và mặc định không ghi history.
- Privacy Guard bỏ qua password manager cấu hình sẵn và clipboard có format `ExcludeClipboardContentFromMonitorProcessing`.
- API key/token chỉ lưu trong SQLite cục bộ. Tính năng nào cần mạng đều tách khỏi search mặc định và chỉ chạy khi đúng keyword.
- Run as administrator, kill process và uninstall là hành động rõ ràng trong Context Menu; launcher không tự nâng quyền toàn bộ tiến trình.

### Build, kiểm thử và phát hành

- Frontend: `tsc` kiểm kiểu rồi Vite tạo bundle production. Backend: Rust unit tests kiểm parser/hotkey, UTF-8 PowerShell, bộ lọc search và query Everything fallback.
- `scripts/capture-readme.ps1` chạy bản release với profile tạm, thao tác launcher thật bằng hotkey, chụp từng tính năng và kiểm tra kích thước/foreground window; lỗi UI trong lúc chụp khiến script dừng.
- Release Rust bật `panic=abort`, LTO, một codegen unit, `opt-level=s` và strip symbol để giảm dung lượng.
- GitHub Actions trên `windows-latest` cài Node 20 + Rust stable, cache Cargo, build Tauri/NSIS; push `main` cập nhật nightly, tag `v*` tạo release chính thức.

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
