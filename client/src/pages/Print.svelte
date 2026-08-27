<script lang="ts">
  import {
    PrintError,
    fetchFonts,
    postPreview,
    postPrint,
    type Preview,
    type PrintOptions,
  } from "../lib/api";

  type PrintState = "idle" | "printing" | "success" | "error";
  type PreviewState = "idle" | "loading" | "ready" | "error";

  /// Tape the preview assumes; the printer reports the real one when printing.
  const TAPE_MM = 12;
  const PREVIEW_DELAY_MS = 300;

  let text = $state("");
  let offsetPercent = $state(5);
  let fontScalePercent = $state(100);
  let font = $state<string | null>(null);
  let fonts = $state<string[]>([]);

  let printState = $state<PrintState>("idle");
  let errorMessage = $state("");
  let previewState = $state<PreviewState>("idle");
  let preview = $state<Preview | undefined>();
  let previewError = $state("");

  const printable = $derived(text.trim().length > 0);
  const options = $derived<PrintOptions>({
    text,
    offset_percent: offsetPercent,
    font,
    font_scale_percent: fontScalePercent,
  });

  $effect(() => {
    void fetchFonts()
      .then((catalog) => {
        fonts = catalog.fonts;
        font = catalog.default;
      })
      .catch(() => {
        // Without the catalog the server default font still applies.
      });
  });

  // Re-render the preview a moment after the last change. Only the newest
  // request may touch the preview state: older responses are dropped.
  let previewGeneration = 0;
  $effect(() => {
    const current = options;
    const generation = ++previewGeneration;
    if (!printable) {
      previewState = "idle";
      preview = undefined;
      return;
    }
    const timer = setTimeout(() => {
      previewState = "loading";
      postPreview(current, TAPE_MM)
        .then((result) => {
          if (generation !== previewGeneration) return;
          preview = result;
          previewState = "ready";
        })
        .catch((error) => {
          if (generation !== previewGeneration) return;
          previewError =
            error instanceof PrintError
              ? error.message
              : "プレビューを取得できませんでした";
          previewState = "error";
        });
    }, PREVIEW_DELAY_MS);
    return () => clearTimeout(timer);
  });

  async function print() {
    if (!printable || printState === "printing") {
      return;
    }
    printState = "printing";
    try {
      await postPrint(options);
      printState = "success";
    } catch (error) {
      errorMessage =
        error instanceof PrintError
          ? error.message
          : "印刷要求を送れませんでした";
      printState = "error";
    }
  }
</script>

<form
  class="content print"
  data-state={printState}
  onsubmit={(event) => {
    event.preventDefault();
    void print();
  }}
>
  <label class="field">
    <span class="caption">ラベルの文字</span>
    <textarea
      class="input"
      rows="4"
      placeholder="改行で複数行になります"
      bind:value={text}
      disabled={printState === "printing"}></textarea>
  </label>

  <div class="settings">
    <label class="field">
      <span class="caption">フォント</span>
      <select
        class="input"
        bind:value={font}
        disabled={printState === "printing"}
      >
        {#each fonts as id (id)}
          <option value={id}>{id}</option>
        {/each}
      </select>
    </label>
    <label class="field">
      <span class="caption">オフセット (%)</span>
      <input
        class="input"
        type="number"
        min="0"
        max="49"
        bind:value={offsetPercent}
        disabled={printState === "printing"}
      />
    </label>
    <label class="field">
      <span class="caption">文字サイズ (%)</span>
      <input
        class="input"
        type="number"
        min="10"
        max="100"
        bind:value={fontScalePercent}
        disabled={printState === "printing"}
      />
    </label>
  </div>

  <div class="preview" data-preview={previewState}>
    <span class="caption">プレビュー ({TAPE_MM}mm テープ想定)</span>
    <div class="tape">
      {#if previewState === "ready" && preview}
        <img
          alt="ラベルのプレビュー"
          src={`data:image/png;base64,${preview.png_base64}`}
          style={`height: ${preview.height_px}px`}
        />
      {:else if previewState === "loading"}
        <span class="status"
          ><span class="spinner" aria-hidden="true"></span>描画中…</span
        >
      {:else if previewState === "error"}
        <span class="status error">{previewError}</span>
      {:else}
        <span class="status">文字を入力するとここに表示されます</span>
      {/if}
    </div>
    {#if previewState === "ready" && preview}
      <span class="caption">
        長さ 約 {preview.length_mm} mm ({preview.width_px} px、裁断前・機械の余白を除く)
      </span>
    {/if}
  </div>

  <div class="actions">
    <button
      class="btn primary"
      type="submit"
      disabled={printState === "printing"}
    >
      印刷
    </button>
    {#if printState === "printing"}
      <span class="status" role="status">
        <span class="spinner" aria-hidden="true"></span>印刷中…
      </span>
    {:else if printState === "success"}
      <span class="status" role="status">印刷しました</span>
    {/if}
  </div>
  {#if printState === "error"}
    <p class="error-banner" role="alert">{errorMessage}</p>
  {/if}
</form>

<style lang="sass">
  .print
    display: flex
    flex-direction: column
    gap: var(--sp-3)

  .field
    display: flex
    flex-direction: column
    gap: var(--sp-1)
    min-width: 0

  .settings
    display: grid
    grid-template-columns: minmax(0, 2fr) minmax(0, 1fr) minmax(0, 1fr)
    gap: var(--sp-3)

  @media (max-width: 767px)
    .settings
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr)

      .field:first-child
        grid-column: 1 / -1

  .caption
    font-size: var(--fs-xs)
    color: var(--c-muted)

  .input
    width: 100%
    padding: var(--sp-2)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-sm)
    background: var(--c-surface)
    color: var(--c-on-surface)
    font: inherit
    line-height: 1.6

    &:focus
      border-color: var(--c-accent)

    &:disabled
      opacity: 0.5

  textarea.input
    resize: vertical

  .preview
    display: flex
    flex-direction: column
    gap: var(--sp-1)

  .tape
    display: flex
    align-items: center
    min-height: 76px
    padding: var(--sp-2)
    overflow-x: auto
    border: 1px solid var(--c-border)
    border-radius: var(--radius-sm)
    background: var(--c-surface-raised)

    img
      display: block
      max-width: none
      image-rendering: pixelated
      background: #fff

  .actions
    display: flex
    align-items: center
    gap: var(--sp-3)

  .primary
    border-color: var(--c-accent)
    background: var(--c-accent)
    color: var(--c-surface-raised)

    &:hover
      background: var(--c-accent)

    &:disabled
      opacity: 0.5
      cursor: default

  .status
    font-size: var(--fs-sm)
    color: var(--c-muted)

  .status.error
    color: var(--c-danger)

  .error-banner
    margin: 0
    padding: var(--sp-2)
    border-radius: var(--radius-sm)
    background: var(--c-danger-subtle)
    color: var(--c-danger)
    font-size: var(--fs-sm)
    white-space: pre-wrap
</style>
