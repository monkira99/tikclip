# TikClip

Ứng dụng desktop **Tauri 2** (Rust + React/TypeScript) để theo dõi tài khoản TikTok Live, ghi luồng, tách clip và quản lý metadata. **Phase 1 (MVP)** đã có shell UI, SQLite phía Rust, sidecar Python (FastAPI) cho ghi hình/FFmpeg và xử lý clip (PySceneDetect), cùng WebSocket cho sự kiện realtime.

## Kiến trúc tóm tắt

| Thành phần | Vai trò |
|------------|---------|
| **Frontend** (`src/`) | React 19, Vite, shadcn/ui, Zustand — gọi Tauri commands + HTTP/WS tới sidecar |
| **Tauri / Rust** (`src-tauri/`) | SQLite, tray, khởi động sidecar, lệnh CRUD accounts/clips/settings |
| **Sidecar** (`sidecar/`) | FastAPI, health/recording/accounts/clips API, watcher live, worker FFmpeg, processor scene-detect |

Sidecar được spawn tự động khi mở app (`python3 -m main` với `PYTHONPATH` trỏ tới `sidecar/src`). Cổng HTTP được in ra stdout dạng `SIDECAR_PORT=<n>`; UI dùng cổng đó cho REST và WebSocket.

## Yêu cầu hệ thống

- **Node.js** 20+ (khuyến nghị LTS) và npm
- **Rust** stable + **Cargo** (cài qua [rustup](https://rustup.rs/))
- **Python** 3.11+ trên `PATH` với tên lệnh **`python3`** (Tauri gọi `python3`)
- **[uv](https://docs.astral.sh/uv/)** (khuyến nghị) để cài dependency và chạy sidecar/tests
- **FFmpeg** và **ffprobe** trên `PATH` (ghi stream, probe duration, cắt clip, thumbnail)
- **OpenCV** (qua gói `scenedetect[opencv]`) cho PySceneDetect khi chạy xử lý cảnh

### macOS (Homebrew)

```bash
brew install node python@3.12 ffmpeg
# Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# uv: curl -LsSf https://astral.sh/uv/install.sh | sh
```

### Kiểm tra nhanh

```bash
node -v && npm -v && cargo -V && python3 --version && ffmpeg -version | head -1 && ffprobe -version | head -1
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

Đảm bảo khi chạy app desktop, lệnh `python3` tìm thấy môi trường đã cài `tikclip-sidecar` (global hoặc `uv`/`venv` được kích hoạt trong shell **không** áp dụng cho process con của Tauri — thường cần `python3` trỏ tới interpreter đã `pip install -e sidecar/`).

**Gợi ý ổn định:** cài package editable vào user site hoặc dùng `uv tool` / shim sao cho `python3` mặc định là interpreter có dependency sidecar.

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
| `TIKCLIP_STORAGE_PATH` | `~/TikTokApp` | Thư mục lưu raw/clips |
| `TIKCLIP_POLL_INTERVAL_SECONDS` | `30` | Chu kỳ watcher kiểm tra live |
| `TIKCLIP_LOG_LEVEL` | `info` | Log uvicorn |

Có thể đặt trong `sidecar/.env` khi chạy tay với `uv run --env-file .env` (không commit secret).

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
