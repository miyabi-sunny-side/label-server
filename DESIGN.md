---
version: alpha
name: Sumi / label-server
description: >
  Self-contained design contract for label-server — a two-mode tool
  that prints Brother PT-P700 labels from a textarea. Dark theme is
  Sumi (the CSS default), light theme is Kinari; Washi is deliberately
  not adopted. Consulted: rust-svelte-template (Sumi + Kinari) @
  2026-08-27. This file is the sole ongoing styling authority for this
  repository.
colors:
  # Kinari (light) palette — the set designmd validates. designmd has no
  # theme concept, so the Sumi (dark) counterpart of every token lives in
  # the Colors section below (Kinari / Sumi pairs) and is implemented in
  # client/src/global.sass. `primary` duplicates `accent` because designmd
  # requires a key color named primary; the family vocabulary is "accent".
  # The identity color is tape teal (Kinari #0f6e64 / Sumi #3fc9b8).
  primary: "#0f6e64"
  accent: "#0f6e64"
  accent-subtle: "rgba(15, 110, 100, 0.10)"
  surface: "#faf6ef"
  surface-raised: "#fffdf8"
  on-surface: "#3a2f28"
  muted: "#6f6257"
  border: "#e3d9c9"
  scrim: "rgba(58, 47, 40, 0.4)"
  link: "#14506e"
  danger: "#9c2b1d"
  danger-subtle: "#f9e9e4"
  # Sprinkle indirection hooks (see Colors): neutral in Sumi, accent wash
  # in Kinari. Components consume these, never accent-subtle directly,
  # for band/hover jobs.
  wash-base: "#eaf3f0"
  wash-raised: "#f3f8f6"
  hover-1: "rgba(15, 110, 100, 0.10)"
  hover-2: "rgba(15, 110, 100, 0.16)"
typography:
  title:
    fontFamily: system-ui
    fontSize: 17px
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: system-ui
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.6
  body-sm:
    fontFamily: system-ui
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: system-ui
    fontSize: 15px
    fontWeight: 500
    lineHeight: 1.2
  caption:
    fontFamily: system-ui
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.4
rounded:
  sm: 6px
  md: 8px
  lg: 12px
  full: 9999px
spacing:
  sp-1: 4px
  sp-2: 8px
  sp-3: 12px
  sp-4: 16px
  sp-5: 24px
components:
  # Quiet controls (button-quiet, icon-button) render with a transparent
  # background at runtime; the backgroundColor below is the backdrop they
  # typically sit on, so contrast is checked against it.
  app-header:
    backgroundColor: "{colors.wash-base}"
    textColor: "{colors.on-surface}"
    height: 48px
  hairline:
    backgroundColor: "{colors.border}"
    height: 1px
  button:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.on-surface}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: 8px
  button-hover:
    backgroundColor: "{colors.hover-1}"
  button-pressed:
    backgroundColor: "{colors.hover-2}"
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.surface-raised}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: 8px
  button-quiet:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.muted}"
    rounded: "{rounded.sm}"
  icon-button:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.sm}"
    size: 36px
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.on-surface}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: 8px
  tape-preview:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.muted}"
    rounded: "{rounded.sm}"
    padding: 8px
  modal:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.on-surface}"
    rounded: "{rounded.lg}"
    padding: 16px
  modal-scrim:
    backgroundColor: "{colors.scrim}"
  radio-selected:
    backgroundColor: "{colors.accent-subtle}"
    rounded: "{rounded.sm}"
  mode-tab:
    backgroundColor: "{colors.wash-base}"
    textColor: "{colors.muted}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: 8px
  # Like radio-selected, this is a translucent tint over the header band;
  # the ink stays on-surface (see Components).
  mode-tab-current:
    backgroundColor: "{colors.accent-subtle}"
    rounded: "{rounded.sm}"
  error-banner:
    backgroundColor: "{colors.danger-subtle}"
    textColor: "{colors.danger}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.sm}"
    padding: 8px
  spinner:
    textColor: "{colors.accent}"
    size: 18px
---

# label-server — Sumi Family

## Overview

label-server is a **two-mode tool**: type text into a textarea, press
印刷, and a Brother PT-P700 prints the label. There is nothing to
browse, configure, or lay out — the whole product is the shortest path
from "I want this text on tape" to a printed label. Everything the UI
adds beyond that path is friction.

The two modes exist because the printer feeds a leader of roughly 24mm
at the start of every job, and that leader is thrown away:

- **個別 (`/`)** — one label per press, with a live preview.
- **連続 (`/continuous`)** — several labels in one job. The leader is
  fed once for the whole batch instead of once per label, so printing
  ten labels wastes one leader rather than ten. Labels are cut apart
  from each other as usual.

The modes are peers, not a hierarchy: each is a single screen, and
switching between them is the only navigation in the product.

The personality is **calm, quiet, and tool-like**: content first, chrome
recedes into neutral ink tones, color only where it means something. The
audience is one engineer who prints labels next to a terminal; density
is welcome, onboarding is not.

Two named themes with fixed jobs:

- **Sumi (墨) — dark, the default.** `:root` IS Sumi. Design here first.
- **Kinari (生成り) — light, for screens.** Warm cream surfaces, sepia
  ink, and a limited license to decorate with faint accent washes.

**Washi is deliberately not adopted.** This tool targets ordinary
screens.

This document is **self-contained**: it was bootstrapped from the
rust-svelte-template Sumi family contract (consulted 2026-08-27) but
depends on nothing outside this repository.

### Domain model

- **Print mode** — 個別 or 連続, chosen by the header tabs and carried
  in the URL. Each mode owns its own form; they share nothing but the
  print options.
- **Label text (個別)** — the textarea contents. Each line becomes one
  printed line; leading and trailing blank lines are dropped, inner
  blank lines are kept. Blank text is not printable.
- **Label list (連続)** — the textarea contents, one label per line.
  Blank lines make no label. A **header word** and a **connector**
  (改行 / 半角スペース / 無し) prefix every label, so 「M4」+ 半角スペース
  over 皿8 / 皿10 prints 「M4 皿8」 and 「M4 皿10」. The server owns the
  joining rule; the UI only says how many labels the job will print.
- **Print options** — font (one of the server's catalog, default
  preselected), 揃え (left / center / right placement of shorter lines,
  default 左寄せ), offset (% of the tape width kept blank on each edge,
  0–49, default 5) and 文字サイズ (% of the auto-fitted size, 10–100,
  default 100). The same options go to the preview and to the print.
- **Preview** — the server renders the exact bitmap it would print, for
  an assumed 12mm tape, and the UI shows it 1:1 with the tape length it
  occupies (before the printer's leader and cut margins). It refreshes
  300ms after the last change; blank text shows no preview.
- **Print request** — one `POST /api/print` (個別) or one
  `POST /api/print/continuous` (連続) per press; the continuous request
  carries every label of the job. The cut mode and the real tape width
  come from the printer.
- **Print state** — `idle | printing | success | error`, exposed on the
  form as `data-state`. Exactly one request is in flight at a time.

## Colors

Every color is a CSS custom property (`--c-*`); components never hardcode
hex. The frontmatter carries the Kinari (light) palette; the Sumi (dark)
counterpart of every token is listed below as a Kinari / Sumi pair and
implemented in `client/src/global.sass`.

- **Surface (#faf6ef / #191919):** page background. Warm cream / ink
  off-black — never pure white or pure black.
- **Surface Raised (#fffdf8 / #232323):** modals, menus, buttons.
- **On-Surface (#3a2f28 / #e6e6e6):** primary text. ~11:1 on Kinari
  surface, comfortably AA+ on Sumi.
- **Muted (#6f6257 / #9a9a9a):** field captions, status text, quiet
  icons. ≥ 4.5:1 (AA) against surface in both themes.
- **Border (#e3d9c9 / #333333):** 1px hairlines — the primary separation
  tool of this flat system.
- **Accent (#0f6e64 / #3fc9b8) — tape teal, the project identity.**
  Marks the primary action (印刷), the focus ring, the focused textarea
  border, the selected theme radio, and the spinner — "this is the main
  move". One accent-filled element per screen. The selected-state tint
  is `accent-subtle` (rgba(15,110,100,.10) / rgba(63,201,184,.15)).
- **Link (#14506e / #7fdbff)**, **Danger (#9c2b1d / #ff6b6b)** with
  `danger-subtle` tints (#f9e9e4 / #3a1a1a) for the printer error
  banner.
- **Scrim (rgba(58,47,40,.4) / rgba(0,0,0,.6)):** modal backdrop.

There are no domain data colors: label text is monochrome on tape, and
the UI never colors it.

**Sprinkle indirection (the Kinari license, made mechanical).** Four
semantic hooks decouple "where warmth appears" from component code:

| Hook              | Job                                    | Sumi resolves to        | Kinari resolves to    |
| ----------------- | -------------------------------------- | ----------------------- | --------------------- |
| `--c-wash-base`   | app-header band background             | `#232323` (raised)      | `#eaf3f0` (teal wash) |
| `--c-wash-raised` | reserved for sticky bands (none today) | `#191919` (page)        | `#f3f8f6`             |
| `--c-hover-1`     | hover fill (buttons, menu items)       | `#333333` (border gray) | `rgba(15,110,100,.10)` |
| `--c-hover-2`     | pressed / active fill                  | `#3d3d3d`               | `rgba(15,110,100,.16)` |

Components consume the hook, never `accent-subtle` directly, for these
jobs. Sumi stays strictly neutral; Kinari warms up with zero
per-component branching. Washes are decoration only — every meaning they
touch must also be carried by text or shape.

**Theme mechanism.** `:root` carries the Sumi values and
`color-scheme: dark`. Kinari is applied by two equivalent blocks (kept
identical via one Sass mixin), each also setting `color-scheme: light`:

- `:root[data-theme="light"]` — explicit user choice;
- `@media (prefers-color-scheme: light)` → `:root:not([data-theme="dark"])`
  — OS decides when no explicit choice is set.

`data-theme` on `<html>` takes `"dark"` or `"light"`; the auto setting
**removes the attribute** (and the storage key) so the OS rules.
Preference persists in `localStorage` under `label-server:theme` and is
applied before first paint.

The primary button sets its text with the `surface-raised` token, so it
is dark-on-teal in Sumi and warm-white-on-teal in Kinari (≥ 4.5:1) with
no extra token. All text keeps WCAG AA in both themes.

## Typography

One typeface — the platform `system-ui` stack. No webfonts. Exactly five
roles, exposed as font-size tokens `--fs-xs..xl` (12/14/15/16/17px):

- **Title (`--fs-xl` 17px / 600 / 1.3):** modal headers.
- **Body (`--fs-lg` 16px / 400 / 1.6):** the textarea, select and number
  inputs. Never smaller — what is typed is what gets printed, so it must
  be readable.
- **Body Small (`--fs-sm` 14px / 400 / 1.5):** status text and the error
  banner.
- **Label (`--fs-md` 15px / 500 / 1.2):** buttons, menu items, the app
  title.
- **Caption (`--fs-xs` 12px / 400 / 1.4):** field captions above every
  input and the preview's tape-length line — always `muted`.

If a new size feels needed, use weight or muted color instead.

## Layout

The shell stacks two rows:

1. **App header — invariant.** Sticky, 48px, full width,
   `--c-wash-base` background, 1px bottom hairline. Contents are exactly
   three: the app title as a home link (`<a href="/">`, label type,
   on-surface ink, no underline — left), the mode tabs next to it, and
   the hamburger icon-button (right). **Navigation between the two
   modes lives inline in the header; the hamburger menu stays for
   secondary actions only.**
2. **Main content — the form of the current mode**, the only scrolling
   region.

One breakpoint: **768px**. Below it, a single column with `--sp-3` side
gutters; at and above, the content column centers at max-width 720px with
`--sp-5` gutters. Bands stay full-width at all widths. The page never
scrolls horizontally at 320px and up — the preview strip scrolls
internally when a label is wider than the column.

Spacing snaps to the 4px scale `--sp-1..5` (4/8/12/16/24px). The form
stacks its rows with `--sp-3` gaps; caption-to-field is `--sp-1`; the
settings row is a grid (font 2fr, then 揃え / offset / size 1fr each;
below 768px the font spans a full row and the other three flow in two
columns) with `--sp-3` gaps; the
action row lays out button and status with `--sp-3`. No off-scale
values.

## Elevation & Depth

The system is **flat**. Hierarchy comes from tonal layers (surface →
surface-raised → wash band) plus 1px hairlines. Exactly one shadow
exists: floating modals/menus cast `0 8px 32px rgba(0, 0, 0, 0.25)` over
the scrim. No other `box-shadow` anywhere.

**Focus ring:** defined once globally on `:focus-visible` —
`outline: 2px solid var(--c-accent); outline-offset: 2px`. The UA
default ring is suppressed only because this replaces it; focus
indication is never removed outright.

## Shapes

Soft-rectangle language, tokens `--radius-sm/md/lg/full` (6/8/12/9999px):

- **sm (6px):** buttons, every input, the preview strip, the error
  banner, all small controls.
- **md (8px):** reserved (no list rows today).
- **lg (12px):** modals and floating menus.
- **full:** the spinner only.

Never mix radii within one composite control. No circular buttons.

## Iconography

All icons come from **one dictionary component**,
`client/src/lib/Icon.svelte`: `<Icon name="menu" />` renders inline SVG
on a 24×24 grid — `fill="none" stroke="currentColor" stroke-width="2"
stroke-linecap="round" stroke-linejoin="round"` (Lucide style), default
size `1.2em`, baseline-aligned, inheriting the text color of its context.

The dictionary was copied whole from the family template and is kept
whole: `menu`, `x`, `sun`, `moon`, `monitor`, `chevron-left`, `trash`,
`megaphone`, `megaphone-off`, `pencil`, `refresh-cw`, `check-check`,
`mail`, `book`, `search`, `star`, `star-filled`. Only `menu`, `x`,
`sun`, `moon`, `monitor` are used today; the rest stay as vocabulary.
`Icon.svelte` also exports `ICON_NAMES`, the canonical array of every
entry.

- **Emoji are banned as UI icons**, and so are text glyphs standing in
  for icons (▲ ▼ × ☰ ▶ …) — always an SVG entry in the dictionary.
- A new icon is added to this project's `Icon.svelte` in the same
  grammar; nothing here depends on the family template at build or run
  time.

## Components

- **App header:** per Layout. The title link keeps on-surface ink with
  no underline (chrome, not content). The hamburger is a 36px quiet
  icon-button with `aria-label` and `aria-expanded`.
- **Mode tabs:** a `<nav aria-label="印刷モード">` sitting immediately
  right of the title and taking the remaining space before the
  hamburger. Two `<a>` rows — 個別 (`/`) and 連続 (`/continuous`) —
  label type, `--sp-2`/`--sp-3` padding, sm radius, `--sp-1` between
  them, muted ink, hover `--c-hover-1`. The current one carries
  `aria-current="page"` and takes the same `accent-subtle` tint as a
  selected theme radio with on-surface ink. They stay plain links, so
  middle-click and ⌘-click open a real tab; a plain click is
  intercepted and becomes a `pushState`.
- **Menu (from the hamburger):** a dropdown panel spatially anchored to
  the hamburger, not a modal — absolutely positioned at `top: 100%` /
  `right: 0` within the header's positioned right slot, `min-width`
  180px, surface-raised background, 1px hairline border, lg radius with
  `overflow: hidden`, and the single floating shadow. There is **no
  scrim**; a transparent `position: fixed` full-viewport close button
  sits behind the panel so any outside click closes it. Esc also
  closes; closing always returns focus to the hamburger, and
  `aria-expanded` mirrors the open state. Items are full-width
  borderless rows — label type, `--sp-2`/`--sp-3` padding, left
  aligned, transparent background, hover `--c-hover-1`. **The only
  item is テーマ設定**, which opens the centered theme settings modal.
- **Theme settings modal:** opened from the menu's テーマ設定 item; the
  centered modal (lg radius, 16px padding, scrim + shadow) holding a
  `role="radiogroup"` with three radios — 自動 (`monitor`), ライト
  (`sun`), ダーク (`moon`). Selecting applies immediately (attribute +
  storage) and **does not close the modal**. Close via ×, Esc, or scrim;
  focus returns to the hamburger.
- **Print form — the 個別 page (`/`).** A `<form>` carrying
  `data-state="idle|printing|success|error"`:
  - _field:_ a `<label>` wrapping the caption ラベルの文字 (caption,
    muted) and the textarea (input recipe: surface bg, 1px hairline,
    sm radius, 8px padding, body type, 4 visible rows, vertical resize
    only; focus swaps the border to accent under the shared focus ring).
    Placeholder reads 改行で複数行になります.
  - _settings row:_ four labelled inputs in the input recipe — フォント
    (`<select>` listing the catalog ids, default preselected), 揃え
    (`<select>` 左寄せ / 中央寄せ / 右寄せ), オフセット (%)
    (`type="number"` 0–49) and 文字サイズ (%) (`type="number"` 10–100).
    Every change re-renders the preview.
  - _preview:_ caption プレビュー (12mm テープ想定), then the tape strip
    (tape-preview recipe: surface-raised bg, 1px hairline, sm radius, 8px
    padding, min-height 76px, horizontal scroll) carrying
    `data-preview="idle|loading|ready|error"`: _idle_ shows the muted
    hint 文字を入力するとここに表示されます; _loading_ the accent spinner
    + 描画中…; _ready_ the rendered label as an `<img alt="ラベルのプレビュー">`
    at 1 CSS px per printed px on a white label ground; _error_ the
    server's message in danger. Below a ready preview a caption reads
    長さ 約 N mm (W px、裁断前・機械の余白を除く).
  - _action row:_ the primary button 印刷 (`type="submit"`, accent bg,
    surface-raised text) followed by the status slot. Enter inside the
    textarea inserts a newline; the button is the only way to submit.
  - _idle:_ status slot empty.
  - _printing:_ button and textarea disabled (50% opacity), status shows
    the accent spinner + 印刷中… as `role="status"`. A second press
    while printing does nothing — one request in flight.
  - _success:_ controls re-enabled, status reads 印刷しました
    (body-sm muted), the text stays in the textarea so the same label
    can be printed again.
  - _error:_ controls re-enabled, an error banner (`role="alert"`,
    danger-subtle bg, danger text, body-sm, sm radius, 8px padding,
    `white-space: pre-wrap`) shows the printer's own message verbatim.
  - Blank text (only whitespace) never submits and leaves the state at
    `idle`.
- **Continuous form — the 連続 page (`/continuous`).** A `<form>`
  carrying the same `data-state="idle|printing|success|error"`:
  - _header row:_ two labelled inputs in the input recipe, a 2fr / 1fr
    grid with `--sp-3` gap — ヘッダーワード (`type="text"`, placeholder
    M4) and 接続ワード (`<select>` 改行 / 半角スペース / 無し, default
    半角スペース). Below 768px they stack.
  - _field:_ the caption ラベルの文字 and a 6-row textarea in the same
    input recipe as 個別, placeholder 改行ごとに別のラベルになります.
  - _settings row:_ the same four inputs as 個別, from one shared
    component — the two modes never drift apart.
  - _action row:_ the primary button 印刷 followed by a muted
    「N 枚」 count that tracks the non-blank lines, then the status
    slot. The button is disabled while the count is 0 — a job of no
    labels is never sent.
  - _states:_ identical to 個別 (printing disables every control and
    shows the accent spinner + 印刷中…; success reads 印刷しました and
    keeps the text; error shows the printer's message verbatim in the
    banner). There is **no preview** — one strip cannot honestly stand
    for a batch.
- **Buttons:** default = surface-raised bg, 1px hairline, label type,
  sm radius, 8×14px padding, hover fills `--c-hover-1`. Primary =
  accent bg, `surface-raised`-token text — 印刷 is the only one.
  Quiet = transparent, for icon-buttons in bars. Disabled = 50%
  opacity, no pointer.
- **Modals:** centered, lg radius, 16px padding, scrim + the single
  permitted shadow; close via ×, Esc, scrim; content scrolls
  internally, max-height 80dvh.
- **Motion:** utilitarian only — height/opacity transitions ≤ 150ms and
  the spinner. Honor `prefers-reduced-motion: reduce` by disabling both.

## Non-goals

- No tape-width setting — the printer reports the loaded tape when
  printing; the preview assumes 12mm.
- No calibration of the preview against the physical label (leader,
  cut margin, thermal spread) — deliberately deferred.
- No history of printed labels, no templates, no accounts.
- No routing beyond the two modes: `/` and `/continuous` are the whole
  app. Any other path serves the same page and falls back to 個別.
- No preview in 連続 — the preview belongs to the single-label mode.
- No per-label options in a continuous job: every label of one job
  shares one font, alignment, offset and size.

## Implementation Mapping

- Styling is **Sass indented syntax (`.sass`)** with **normalize.css**
  imported first.
- All tokens live in `client/src/global.sass` on `:root` (Sumi values);
  the two equivalent Kinari blocks are emitted from a single Sass mixin
  so they cannot drift.
- Canonical custom-property names: colors `--c-<token>`
  (`--c-surface`, `--c-on-surface`, `--c-accent`, `--c-wash-base`, …),
  spacing `--sp-1..--sp-5`, font sizes `--fs-xs..--fs-xl`, radii
  `--radius-sm/md/lg/full`. Components consume variables only.
- Theme bootstrap script: read `label-server:theme`; `"light"` /
  `"dark"` set `data-theme` on `<html>` before first paint; absent key
  (auto) leaves the attribute off. `Icon.svelte` is the sole icon
  source.
- The 個別 form is `client/src/pages/Print.svelte` and the 連続 form is
  `client/src/pages/Continuous.svelte`; the settings row both use is
  `client/src/lib/Settings.svelte`. Requests go through
  `client/src/lib/api.ts` (`/api/fonts`, `/api/preview`, `/api/print`,
  `/api/print/continuous`).
- Routing is `client/src/lib/routes.ts` (the pattern table) and
  `client/src/lib/router.svelte.ts` (a runes store plus the delegated
  click handler that turns same-origin links into `pushState`).
  `App.svelte` picks the page from `router.index`. The server serves
  `index.html` for unknown paths, so a deep link survives a reload.

## Verification

- `designmd lint` validates the frontmatter structure.
- UI claims in this document are verified **in a real browser** against
  DOM, computed styles, geometry, and operations — never by reading
  source alone. The standing invariants:
  1. Default (no `data-theme`): `color-scheme` is `dark`, body
     background computes to `rgb(25, 25, 25)`.
  2. Choosing ライト in the theme modal sets `data-theme="light"`,
     turns the body `rgb(250, 246, 239)`, writes the storage key, and
     leaves the modal open.
  3. At 375px the header contains exactly four interactive elements —
     the title `<a href="/">`, the two mode tabs and the hamburger
     `<button>` — and `document.documentElement.scrollWidth` never
     exceeds the viewport, with the menu closed or open.
  3b. Loading `/continuous` directly renders the 連続 form with its tab
     marked `aria-current="page"`; clicking 個別 swaps the form without
     a document load and the browser's back button swaps it back.
  4. The form's `data-state` walks idle → printing → success (or error)
     on one press; during printing the 印刷 button and every input are
     disabled and exactly one `POST /api/print` is sent, carrying the
     same font / offset / size as the last preview.
  4a. On the 連続 page, three non-blank lines show 「3 枚」 and one
     press sends exactly one `POST /api/print/continuous` whose
     `bodies` are those three lines, whose `headers` repeat the header
     word, and whose settings match the four inputs. With no non-blank
     line the 印刷 button is disabled.
  4b. Typing text then pausing 300ms sends one `POST /api/preview`; the
     strip's `data-preview` reaches `ready`, the `<img>` height equals
     the returned `height_px`, and the caption states the mm length.
     Blank text returns the strip to `idle` without a request.
  5. Chrome icons are all inline SVG on the 24×24 viewBox grid, stroked
     with `currentColor` and rendered at 1.2em; no emoji or glyph icons
     anywhere.
  6. `:focus-visible` on any control shows the 2px accent outline with
     2px offset; the focused textarea border is the accent color.
  7. Clicking the hamburger opens the dropdown anchored to the header's
     bottom edge and the hamburger's right edge (±1px); computed
     `min-width` 180px, 12px radius, 1px border, the single floating
     shadow; no scrim element exists and `aria-expanded` is `true`. Esc
     closes it and focus returns to the hamburger.

## Do's and Don'ts

- Do source every color from a `--c-*` variable; don't hardcode hex in
  components.
- Do consume `--c-wash-*` / `--c-hover-*` for bands and hovers; don't
  reach for `accent-subtle` directly in those jobs.
- Do keep exactly one accent-filled primary action per screen — 印刷.
- Do present the menu as a hamburger-anchored dropdown; centered
  modals are for dialogs (theme settings), never for navigation.
- Do keep mode switching inline in the header as plain `<a>` links;
  don't move it into the hamburger menu and don't reimplement it as
  buttons that only work with JavaScript.
- Do give both modes the same settings component; don't let the two
  forms grow separate copies of the same four inputs.
- Don't use emoji or text glyphs as icons; every icon is an
  `Icon.svelte` dictionary entry.
- Don't introduce font sizes, radii, spacing values, or shadows outside
  the defined scales — the modal shadow is the only shadow.
- Do give the form all four states; don't ship a page where error
  renders as blank or success is silent.
- Do show the printer's error text verbatim; don't paraphrase it into a
  generic message.
- Do maintain WCAG AA (4.5:1) for all text in both themes; verify in
  the browser, not by eye.
- Do design in Sumi first, then verify Kinari as a warm sibling — never
  as an inverted afterthought.
