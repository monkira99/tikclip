# TikClip

Ứng dụng desktop **Tauri 2** (Rust + React/TypeScript) để theo dõi tài khoản TikTok Live, ghi luồng, tách clip, gợi ý sản phẩm, tạo caption và quản lý metadata. Runtime chính chạy trong Rust/Tauri với SQLite cục bộ.

## Kiến trúc tóm tắt

| Thành phần | Vai trò |
|------------|---------|
| **Frontend** (`src/`) | React 19, Vite, shadcn/ui, Zustand — gọi Tauri commands |
| **Tauri / Rust** (`src-tauri/`) | SQLite, tray, live runtime, recording/clip/audio/caption/product runtime, Gemini embeddings và vector search |

Không còn Python sidecar trong runtime app.

## Yêu cầu hệ thống

- **Node.js** 20+ (khuyến nghị LTS) và npm
- **Rust** stable + **Cargo** (cài qua [rustup](https://rustup.rs/))
- **FFmpeg** và **ffprobe** trên `PATH` (ghi stream, probe duration, cắt clip, thumbnail)

### macOS (Homebrew)

```bash
brew install node ffmpeg
# Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Windows

```powershell
winget install OpenJS.NodeJS.LTS Rustlang.Rustup Gyan.FFmpeg
```

### Linux

```bash
# Debian/Ubuntu
sudo apt update && sudo apt install -y nodejs npm ffmpeg curl build-essential
# Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Kiểm tra nhanh

```bash
node -v && npm -v && cargo -V && ffmpeg -version | head -1 && ffprobe -version | head -1
```

Windows PowerShell:

```powershell
node -v; npm -v; cargo -V; ffmpeg -version; ffprobe -version
```

## Cài đặt

Từ thư mục gốc repo:

```bash
npm install
```

## Chạy ứng dụng (dev)

```bash
npm run tauri dev
```

- Vite chạy tại `http://localhost:1420` (theo `tauri.conf.json`).
- Rust build lần đầu có thể mất vài phút.

## Logging

App mặc định bật log Rust ở mức `info` cho crate `tikclip_lib` và giữ dependency ở mức `warn`.

```bash
# Log chi tiết app, giữ dependency bớt ồn
RUST_LOG=warn,tikclip_lib=info npm run tauri dev

# Debug sâu khi cần điều tra live runtime / TikTok / product vectors
RUST_LOG=warn,tikclip_lib=debug npm run tauri dev
```

## Kiểm thử và chất lượng (nên chạy trước Phase 2)

```bash
# Toàn repo: ESLint + build frontend, rustfmt + clippy
npm run lint
```

- **Lint JS:** `npm run lint:js` (gồm `tsc` + Vite build).
- **Lint Rust:** `npm run lint:rust`.

## Dữ liệu & Phase 1

- **SQLite** (Rust): accounts, clips, settings, runtime telemetry, product metadata và product embedding vectors.
- **Gemini embeddings**: cấu hình qua Settings trong app; sản phẩm cần re-index để tạo vector trong SQLite.
- **E2E:** nên tự kiểm tra mở app, bật flow, ghi live, tách clip và import/index sản phẩm nếu có môi trường TikTok hợp lệ.

## Build bản phát hành

```bash
npm run tauri build
```

Artifact nằm dưới `src-tauri/target/release/` (và thư mục bundle theo nền tảng).

## Cấu trúc thư mục (rút gọn)

```
├── src/                 # React frontend
├── src-tauri/           # Tauri + Rust runtime, SQLite, TikTok/Gemini/product vectors
└── docs/superpowers/    # Spec & kế hoạch phase (chi tiết sản phẩm)
```
