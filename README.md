# TikClip

Ứng dụng desktop **Tauri 2** (Rust + React/TypeScript) để theo dõi tài khoản TikTok Live, ghi luồng, tách clip và quản lý metadata. **Phase 1 (MVP)** đã có shell UI, SQLite phía Rust, sidecar Python (FastAPI) cho ghi hình/FFmpeg và xử lý clip (PySceneDetect), cùng WebSocket cho sự kiện realtime.

## Kiến trúc tóm tắt

| Thành phần | Vai trò |
|------------|---------|
| **Frontend** (`src/`) | React 19, Vite, shadcn/ui, Zustand — gọi Tauri commands + HTTP/WS tới sidecar |
| **Tauri / Rust** (`src-tauri/`) | SQLite, tray, khởi động sidecar, lệnh CRUD accounts/clips/settings |
| **Sidecar** (`sidecar/`) | FastAPI, health/recording/accounts/clips API, watcher live, worker FFmpeg, processor scene-detect |

Sidecar được spawn tự động khi mở app: ưu tiên Python trong **`sidecar/.venv`** (`.venv/bin/python3` trên macOS/Linux, `.venv\Scripts\python.exe` trên Windows), nếu không có thì dùng Python trên `PATH` (`python3`/`python` trên Unix, `python`/`py` trên Windows). Tauri truyền `PYTHONPATH=sidecar/src` và chạy `-m main`. Cổng HTTP in ra stdout dạng `SIDECAR_PORT=<n>`; UI dùng cổng đó cho REST và WebSocket.

## Yêu cầu hệ thống

- **Node.js** 20+ (khuyến nghị LTS) và npm
- **Rust** stable + **Cargo** (cài qua [rustup](https://rustup.rs/))
- **Python** 3.11+ (dùng để tạo `sidecar/.venv`; fallback khi không có venv: Python trên `PATH`)
- **[uv](https://docs.astral.sh/uv/)** (khuyến nghị) để cài dependency và chạy sidecar/tests
- **FFmpeg** và **ffprobe** trên `PATH` (ghi stream, probe duration, cắt clip, thumbnail)
- **OpenCV** (qua gói `scenedetect[opencv]`) cho PySceneDetect khi chạy xử lý cảnh

### macOS (Homebrew)

```bash
brew install node python@3.12 ffmpeg
# Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# uv: curl -LsSf https://astral.sh/uv/install.sh | sh
```

### Windows

```powershell
winget install OpenJS.NodeJS.LTS Python.Python.3.12 Rustlang.Rustup Gyan.FFmpeg Astral.UV
```

### Linux

```bash
# Debian/Ubuntu
sudo apt update && sudo apt install -y nodejs npm python3 python3-venv ffmpeg curl build-essential
# Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# uv: curl -LsSf https://astral.sh/uv/install.sh | sh
```

### Kiểm tra nhanh

```bash
node -v && npm -v && cargo -V && python3 --version && ffmpeg -version | head -1 && ffprobe -version | head -1
```

Windows PowerShell:

```powershell
node -v; npm -v; cargo -V; python --version; ffmpeg -version; ffprobe -version
```

## Cài đặt

Từ thư mục gốc repo:

```bash
npm install
cd sidecar && uv sync --extra dev && cd ..
```

Nếu không dùng `uv`, trong `sidecar/` có thể dùng:

```bash
python3 -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install -e ".[dev]"
```

Windows PowerShell:

```powershell
py -3 -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -e ".[dev]"
```

**Bắt buộc cho dev ổn định:** chạy `cd sidecar && uv sync` (hoặc `pip install -e ".[dev]"` trong `.venv`) để có thư mục **`sidecar/.venv`**. App Tauri sẽ ưu tiên interpreter này — không phụ thuộc shell đang bật venv. Nếu không có `.venv`, app dùng Python hệ thống (phải tự cài đủ dependency sidecar).

## Chạy ứng dụng (dev)

```bash
npm run tauri dev
```

- Vite chạy tại `http://localhost:1420` (theo `tauri.conf.json`).
- Rust build lần đầu có thể mất vài phút.

## Chạy sidecar riêng (debug)

Khi cần log API/WS tách khỏi Tauri:

```bash
cd sidecar
PYTHONPATH=src uv run --extra dev --env-file .env python -m main
# Không có .env: PYTHONPATH=src uv run --extra dev python -m main
# Hoặc: PYTHONPATH=src python3 -m main  (sau khi pip install -e ".[dev]")
```

Windows PowerShell:

```powershell
cd sidecar
$env:PYTHONPATH = "src"
uv run --extra dev --env-file .env python -m main
```

Ứng dụng in một dòng `SIDECAR_PORT=...` rồi phục vụ HTTP trên cổng đó.

## Kiểm thử và chất lượng (nên chạy trước Phase 2)

```bash
# Toàn repo: ESLint + build frontend, Ruff + ty + format Python, rustfmt + clippy
npm run lint

# Chỉ tests Python
cd sidecar && uv run pytest tests/ -q
```

- **Lint JS:** `npm run lint:js` (gồm `tsc` + Vite build).
- **Lint Python:** `npm run lint:py` (cần `uv` trong `PATH`).
- **Lint Rust:** `npm run lint:rust`.

## Cấu hình sidecar (biến môi trường)

Prefix **`TIKCLIP_`** (xem `sidecar/src/config.py`). Một số biến thường dùng:

| Biến | Mặc định | Ý nghĩa |
|------|----------|---------|
| `TIKCLIP_HOST` | `127.0.0.1` | Bind HTTP |
| `TIKCLIP_PORT` | `18321` | Cổng ưu tiên (có dải fallback nếu bận) |
| `TIKCLIP_STORAGE_PATH` | `~/.tikclip` | Thư mục lưu raw/clips |
| `TIKCLIP_POLL_INTERVAL_SECONDS` | `30` | Chu kỳ watcher kiểm tra live |
| `TIKCLIP_LOG_LEVEL` | `info` | Mức log cho logger `tikclip.*` (vd. `debug` để trace TikTok) |
| `TIKCLIP_DEBUG_TIKTOK` | `false` | Nếu `true`: khi không parse được `room_id` từ trang live, log một đoạn HTML rút gọn (không log cookie) |

Log sidecar dạng `tikclip.tiktok` / `tikclip.watcher` / `tikclip.routes.accounts` in ra **stderr** (Terminal khi chạy `tauri dev` hoặc process Python).

Mẫu biến: `sidecar/.env.example` — copy thành `sidecar/.env`. Sidecar **tự đọc** `sidecar/.env` khi khởi động (kể cả khi Tauri spawn Python); biến môi trường của process vẫn **ghi đè** giá trị trong file. Không commit `.env`.

## Dữ liệu & Phase 1

- **SQLite** (Rust): accounts, clips mirror, settings — đường dẫn DB do Tauri quản lý trong profile app.
- **Phase 1:** luồng end-to-end cơ bản (UI ↔ Tauri ↔ sidecar) đã được dựng; nên **tự kiểm tra E2E** (mở app, sidecar kết nối, thử API/tính năng ghi nếu có môi trường TikTok hợp lệ) trước khi mở rộng.
- **Đồng bộ cài đặt** giữa SQLite UI và env `TIKCLIP_*` của sidecar có thể chưa hoàn chỉnh — ưu tiên env khi chạy sidecar độc lập.

## Build bản phát hành

```bash
npm run tauri build
```

Artifact nằm dưới `src-tauri/target/release/` (và thư mục bundle theo nền tảng).

## Cấu trúc thư mục (rút gọn)

```
├── src/                 # React frontend
├── src-tauri/           # Tauri + Rust (SQLite, sidecar manager)
├── sidecar/             # Python FastAPI sidecar
│   ├── pyproject.toml
│   └── src/             # PACKAGE: PYTHONPATH trỏ vào đây khi chạy -m main
└── docs/superpowers/    # Spec & kế hoạch phase (chi tiết sản phẩm)
```
