# label-server

Type text into a textarea, press 印刷, and a Brother PT-P700 prints the label. One Rust binary
serves the JSON API and the compiled Svelte page, renders the text, and talks to the printer over
USB itself (libusb is linked statically) — no CUPS, no driver, no external tool.

The USB protocol and the text layout rules are a Rust port of
[ptouch-print](https://codeberg.org/askaaron/ptouch-print) by Dominic Radermacher, which is why
this project is licensed under the GPLv3 (see [License](#license)).

## Prerequisites

- Rust 1.96.0 (the checked-in toolchain file selects it automatically through `rustup`)
- Node.js 24 LTS and npm
- A TrueType / OpenType font with the glyphs you print. Noto Sans CJK is picked up automatically
  from its usual Linux locations, Hiragino Sans on macOS; every font found joins the catalog
  offered in the UI, and `LABEL_FONT` adds one more as the default
- A PT-P700 connected over USB with **Editor Lite turned off** — hold the Editor Lite button for
  about two seconds until its lamp goes out. While the lamp is on, the printer shows up as a USB
  disk (`04f9:2064`) instead of a printer (`04f9:2061`) and nothing can print.
- Permission to open the USB device (membership in the `lp` group is enough on most Linux
  distributions; macOS needs nothing extra)

## Quick start

```sh
cd client
npm ci
npm run build
cd ..
cargo run --locked
```

Open <http://127.0.0.1:3000>, type the label text, press 印刷.

## API

All label options are shared by printing and previewing:

| Field                | Default            | Meaning                                                              |
| -------------------- | ------------------ | -------------------------------------------------------------------- |
| `text`               | required           | One printed line per line of text.                                   |
| `offset_percent`     | `5`                | Blank tape kept on each edge, as % of the tape width (0–49).         |
| `font`               | catalog default    | A font id from `GET /api/fonts`.                                     |
| `font_scale_percent` | `100`              | Shrinks the auto-fitted font size (10–100).                          |

- `POST /api/print` with `{"text": "line 1\nline 2", ...}` — prints one label on the loaded tape
  with the blank leader pre-cut. Returns `200 {"output": "printed <w>x<h>px on <n>mm tape"}` on
  success, `400 {"error": "..."}` for blank text, an unknown font, or an option out of range, and
  `502 {"error": "<reason>"}` when the
  printer is missing, in Editor Lite mode, or the USB transfer fails.
- `POST /api/preview` with the same fields plus `tape_mm` (default 12) — renders exactly what
  `/api/print` would send, without a printer. Returns `{"png_base64", "width_px", "height_px",
  "tape_px", "length_mm"}`; `length_mm` is the tape the label occupies at 180 dpi, before the
  printer's own leader and cut margins.
- `GET /api/fonts` — `{"fonts": ["<id>", ...], "default": "<id>"}`. Ids are the file stems of the
  fonts found at startup.
- `GET /api/health` — `{"status":"ok"}`
- `GET /healthz` — plain-text `ok`

Printing from the shell is the same request:

```sh
curl --fail -X POST http://127.0.0.1:3000/api/print \
  -H 'Content-Type: application/json' \
  -d '{"text":"12mm テスト"}'
```

## Development

Use two terminals so Vite can provide hot module replacement while Rust handles the API.

Terminal 1, from the repository root:

```sh
cargo run
```

Terminal 2:

```sh
cd client
npm ci
npm run dev
```

Open <http://127.0.0.1:5173>. Vite proxies `/api` requests to `http://127.0.0.1:3000`.

## Verify changes

Run the complete local verification set from the repository root:

```sh
npm --prefix client ci
npm --prefix client run format:check
npm --prefix client run check
npm --prefix client test
npm --prefix client run build
npm --prefix client run lint:design
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo build --locked --release
```

The Rust tests never touch a printer: the HTTP layer is tested against a recording `Printer`, and
the USB protocol against a recording `Transport`. The renderer tests read Noto Sans CJK from
`/usr/share/fonts/noto-cjk/`. `npm run lint:design` checks the design contract in
[`DESIGN.md`](DESIGN.md).

## Configuration

| Variable              | Default          | Purpose                                                                   |
| --------------------- | ---------------- | ------------------------------------------------------------------------- |
| `APP_BIND_ADDR`       | `127.0.0.1:3000` | Socket address of the HTTP listener. Use `0.0.0.0:3000` to serve the LAN. |
| `LABEL_FONT`          | unset            | Extra TTF/OTF/TTC to load first (it becomes the default font).            |
| `RUST_LOG`            | `info`           | Logging filter, for example `label_server=debug,tower_http=debug`.        |

## Repository structure

```text
.
├── .github/workflows/  # Continuous integration
├── client/             # Svelte 5 page, Vite config, tests, and the npm lockfile
├── src/                # Axum router, PT-P700 protocol (ptouch.rs), text renderer (render.rs)
├── Cargo.toml          # Rust package and dependency configuration
├── DESIGN.md           # UI design contract
└── rust-toolchain.toml # Pinned Rust toolchain and components
```

The backend reserves `/api/*` for API routes. Unknown API paths return 404 instead of the frontend.
Other unknown paths fall back to `client/dist/index.html`.

## License

label-server is free software under the
[GNU General Public License version 3](LICENSE) (GPL-3.0-only).

`src/ptouch.rs` and `src/render.rs` are derived from ptouch-print,
Copyright (C) 2013-2026 Dominic Radermacher, GPL-3.0. The original is at
<https://git.familie-radermacher.ch/linux/ptouch-print.git>.
