# label-server

Type text into a textarea, press 印刷, and a Brother PT-P700 prints the label. One Rust binary
serves the JSON API and the Svelte page (compiled into it), renders the text with an embedded
font, and talks to the printer over USB itself (libusb is linked statically) — no CUPS, no driver,
no external tool, nothing to place next to the binary.

In the browser only 文字サイズ is out in the open (it opens at 40%, which suits most labels);
フォント・揃え・オフセット・余白 sit behind a 詳細 toggle, closed by default. Values you never
touch are still sent, so a form you never expand prints with the defaults in the table below.

There are two modes. **個別** (`/`) prints one label per press and shows a live preview. **連続**
(`/continuous`) prints a whole list in a single job: the printer feeds its ~24mm leader once for
the batch instead of once per label, so a run of ten labels wastes one leader instead of ten. A
header word can be prefixed to every line — 「M4」 over 皿8 / 皿10 prints 「M4 皿8」 and
「M4 皿10」.

The USB protocol and the text layout rules are a Rust port of
[ptouch-print](https://codeberg.org/askaaron/ptouch-print) by Dominic Radermacher, which is why
this project is licensed under the GPLv3 (see [License](#license)).

## The blank 24mm leader

Every job starts with about 24mm of blank tape, and nothing in software removes it. The PT-P700's
cutter sits ahead of the print head, so the tape between them is already past the head when
printing starts — it can only come out blank. `margin_mm` controls the feed around each label, not
this leader.

What it costs is one blank strip **per job**, not per label. That is what 連続 mode is for: ten
labels sent as one job waste one leader, while ten separate presses waste ten. If you are printing
a batch, put the lines in 連続 and press once.

## Prerequisites

- Rust 1.96.0 (the checked-in toolchain file selects it automatically through `rustup`)
- Node.js 24 LTS and npm
- A PT-P700 connected over USB with **Editor Lite turned off** — hold the Editor Lite button for
  about two seconds until its lamp goes out. While the lamp is on, the printer shows up as a USB
  disk (`04f9:2064`) instead of a printer (`04f9:2061`) and nothing can print.
- Permission to open the USB device (membership in the `lp` group is enough on most Linux
  distributions; macOS needs nothing extra)

## Install

Each [GitHub Release](https://github.com/miyabi-sunny-side/label-server/releases) carries one
self-contained binary per platform plus its checksum:

| Asset                        | Platform                    |
| ---------------------------- | --------------------------- |
| `label-server-linux-x86_64`  | Linux, x86_64 (glibc)       |
| `label-server-macos-aarch64` | macOS, Apple Silicon        |

```sh
curl -LO https://github.com/miyabi-sunny-side/label-server/releases/latest/download/label-server-linux-x86_64
curl -LO https://github.com/miyabi-sunny-side/label-server/releases/latest/download/label-server-linux-x86_64.sha256
sha256sum -c label-server-linux-x86_64.sha256
chmod +x label-server-linux-x86_64
./label-server-linux-x86_64
```

`label-server --version` prints `label-server X.Y.Z` (the release tag without its `v`), which
lets an updater confirm what it downloaded. On macOS use `shasum -a 256 -c` for the checksum. The binaries are **not code-signed**: after
verifying the checksum, clear the quarantine flag once so Gatekeeper lets it run —
`xattr -d com.apple.quarantine label-server-macos-aarch64`. The macOS build is produced by the
release workflow but has not yet been exercised against a printer on a Mac.

Releases are cut by pushing a `v*.*.*` tag. The `Release` workflow runs the same checks as CI,
checks that the tag matches the Cargo package version, builds both binaries natively, and verifies
their checksums, version, health endpoints, embedded SPA and favicon from an empty directory
before attaching them. Main and pull-request CI run checks without building a release binary.
Running the release workflow manually performs the same validation and keeps the binaries as
workflow artifacts without creating a release.

The release profile keeps optimization level 3 and stripping, with LTO disabled and 16 codegen
units to reduce build time. Small binary-size and runtime-memory increases are acceptable for
this service; native Linux/macOS builds and packaged-binary smoke tests remain release checks.

## Quick start (from source)

```sh
cd client
npm ci
npm run build
cd ..
cargo run --locked
```

The client must be built before the server: `cargo build` embeds `client/dist` into the binary and
fails when it is missing. Open <http://127.0.0.1:3000>, type the label text, press 印刷.

Labels are set in **BIZ UDPGothic** (Morisawa's universal-design gothic, compiled into the
binary). Noto Sans CJK and Hiragino Sans are offered as well when the machine has them, and
`LABEL_FONT` adds any TTF/OTF/TTC as the default.

## API

All label options are shared by printing and previewing:

| Field                | Default            | Meaning                                                              |
| -------------------- | ------------------ | -------------------------------------------------------------------- |
| `text`               | required           | One printed line per line of text.                                   |
| `offset_percent`     | `5`                | Blank tape kept on each edge, as % of the tape width (0–49).         |
| `font`               | catalog default    | A font id from `GET /api/fonts`.                                     |
| `font_scale_percent` | `100`              | Shrinks the auto-fitted font size (10–100).                          |
| `align`              | `left`             | Placement of shorter lines: `left`, `center` or `right`.             |
| `margin_mm`          | `2`                | Tape fed before and after every label (2–127). 2mm is the printer's minimum. |

- `POST /api/print` with `{"text": "line 1\nline 2", ...}` — prints one label on the loaded tape
  with the blank leader pre-cut. Returns `200 {"output": "printed <w>x<h>px on <n>mm tape"}` on
  success, `400 {"error": "..."}` for blank text, an unknown font, or an option out of range, and
  `502 {"error": "<reason>"}` when the
  printer is missing, in Editor Lite mode, or the USB transfer fails.
- `POST /api/print/continuous` with `{"headers": [...], "bodies": [...], "connector": "space"}`
  plus the same options — prints every label as one job, cutting between them and ejecting after
  the last. `headers` and `bodies` pair up by index and must be the same length; `connector` is
  `newline`, `space` (default) or `none` and says how each header joins its body; an empty header
  leaves the body alone, and blank bodies print nothing. Same status codes as `/api/print`, plus
  `400` for mismatched lengths or an unknown connector. A batch reports
  `{"output": "printed <n> labels on <n>mm tape"}`, since the labels differ in size.
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
the USB protocol against a recording `Transport`. The renderer tests use the embedded font.
`npm run lint:design` checks the design contract in [`DESIGN.md`](DESIGN.md).

## Configuration

| Variable              | Default          | Purpose                                                                   |
| --------------------- | ---------------- | ------------------------------------------------------------------------- |
| `PORT`                | `3000`           | HTTP port (1–65535), listening on all IPv4 interfaces (`0.0.0.0`). |
| `LABEL_FONT`          | unset            | TTF/OTF/TTC to load first; it becomes the default instead of BIZ UDPGothic. |
| `RUST_LOG`            | `info`           | Logging filter, for example `label_server=debug,tower_http=debug`.        |

`PORT` defaults only when unset. Empty, nonnumeric, or out-of-range values fail startup with an
explicit error. Run `PORT=3010 label-server` to change the port. Deployment and ingress settings
own the access boundary.

## Repository structure

```text
.
├── .github/workflows/  # Continuous integration and the tag-driven release
├── client/             # Svelte 5 page, Vite config, tests, and the npm lockfile
├── src/                # Axum router, PT-P700 protocol (ptouch.rs), text renderer (render.rs)
├── fonts/              # BIZ UDPGothic (OFL) embedded into the binary
├── Cargo.toml          # Rust package and dependency configuration
├── DESIGN.md           # UI design contract
└── rust-toolchain.toml # Pinned Rust toolchain and components
```

The backend reserves `/api/*` for API routes. Unknown API paths return 404 instead of the frontend.
Other unknown paths fall back to the embedded `index.html`.

## License

label-server is free software under the
[GNU General Public License version 3](LICENSE) (GPL-3.0-only).

`src/ptouch.rs` and `src/render.rs` are derived from ptouch-print,
Copyright (C) 2013-2026 Dominic Radermacher, GPL-3.0. The original is at
<https://git.familie-radermacher.ch/linux/ptouch-print.git>.

The embedded font `fonts/BIZUDPGothic-Regular.ttf` is BIZ UDPGothic, Copyright 2022 The BIZ
UDGothic Project Authors, licensed under the SIL Open Font License 1.1
([`fonts/OFL-BIZUDPGothic.txt`](fonts/OFL-BIZUDPGothic.txt)).
