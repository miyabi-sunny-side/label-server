# label-server

Type text into a textarea, press 印刷, and a Brother PT-P700 prints the label. One Rust process
serves both the JSON API and the compiled Svelte page; printing itself is delegated to
[ptouch-print](https://codeberg.org/askaaron/ptouch-print), which talks to the printer over USB
without CUPS.

## Prerequisites

- Rust 1.96.0 (the checked-in toolchain file selects it automatically through `rustup`)
- Node.js 24 LTS and npm
- `ptouch-print` on `PATH` (build it from the link above; it needs `cmake`, `gettext`, `libusb-1.0`
  and `libgd`)
- The fontconfig font `Noto Sans CJK JP` (the server passes this name to `ptouch-print`)
- A PT-P700 connected over USB with **Editor Lite turned off** — hold the Editor Lite button for
  about two seconds until its lamp goes out. While the lamp is on, the printer shows up as a USB
  disk (`04f9:2064`) instead of a printer (`04f9:2061`) and nothing can print.
- Permission to open the USB device (membership in the `lp` group is enough on most distributions;
  ptouch-print also ships udev rules)

Check the printer before starting the server:

```sh
ptouch-print --info
```

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

- `POST /api/print` with `{"text": "line 1\nline 2"}` — prints one label. Each line of `text`
  becomes one printed line; the font is `Noto Sans CJK JP` and the tape is pre-cut. Returns
  `200 {"output": "<ptouch-print stdout>"}` on success, `400 {"error": "text is empty"}` for blank
  text, and `502 {"error": "<ptouch-print stderr>"}` when printing fails.
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

The Rust tests never touch a printer: they point the server at a stub script that records its
arguments. `npm run lint:design` checks the design contract in [`DESIGN.md`](DESIGN.md).

## Configuration

| Variable              | Default          | Purpose                                                                   |
| --------------------- | ---------------- | ------------------------------------------------------------------------- |
| `APP_BIND_ADDR`       | `127.0.0.1:3000` | Socket address of the HTTP listener. Use `0.0.0.0:3000` to serve the LAN. |
| `LABEL_PRINT_COMMAND` | `ptouch-print`   | Command used to print; a path or a name resolved through `PATH`.          |
| `RUST_LOG`            | `info`           | Logging filter, for example `label_server=debug,tower_http=debug`.        |

## Repository structure

```text
.
├── .github/workflows/  # Continuous integration
├── client/             # Svelte 5 page, Vite config, tests, and the npm lockfile
├── src/                # Axum router, ptouch-print invocation, executable entry point
├── Cargo.toml          # Rust package and dependency configuration
├── DESIGN.md           # UI design contract
└── rust-toolchain.toml # Pinned Rust toolchain and components
```

The backend reserves `/api/*` for API routes. Unknown API paths return 404 instead of the frontend.
Other unknown paths fall back to `client/dist/index.html`.

## License

[MIT License](LICENSE).
