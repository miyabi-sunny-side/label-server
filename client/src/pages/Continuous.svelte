<script lang="ts">
  import Settings from "../lib/Settings.svelte";
  import {
    PrintError,
    postContinuousPrint,
    type Align,
    type Connector,
  } from "../lib/api";

  type PrintState = "idle" | "printing" | "success" | "error";

  let header = $state("");
  let connector = $state<Connector>("space");
  let text = $state("");
  let offsetPercent = $state(5);
  // 40% and 60% turned out to be the useful sizes, so the form opens at
  // 40. The API keeps defaulting to 100 when the field is omitted.
  let fontScalePercent = $state(40);
  let marginMm = $state(2);
  let font = $state<string | null>(null);
  let align = $state<Align>("left");

  let printState = $state<PrintState>("idle");
  let errorMessage = $state("");

  // One label per non-blank line; the server joins the header to each body.
  const bodies = $derived(
    text
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0),
  );
  const printable = $derived(bodies.length > 0);

  async function print() {
    if (!printable || printState === "printing") {
      return;
    }
    printState = "printing";
    try {
      await postContinuousPrint({
        headers: bodies.map(() => header.trim()),
        bodies,
        connector,
        offset_percent: offsetPercent,
        font,
        font_scale_percent: fontScalePercent,
        margin_mm: marginMm,
        align,
      });
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
  class="content continuous"
  data-state={printState}
  onsubmit={(event) => {
    event.preventDefault();
    void print();
  }}
>
  <div class="header-row">
    <label class="field">
      <span class="caption">ヘッダーワード</span>
      <input
        class="input"
        type="text"
        placeholder="M4"
        bind:value={header}
        disabled={printState === "printing"}
      />
    </label>
    <label class="field">
      <span class="caption">接続ワード</span>
      <select
        class="input"
        bind:value={connector}
        disabled={printState === "printing"}
      >
        <option value="newline">改行</option>
        <option value="space">半角スペース</option>
        <option value="none">無し</option>
      </select>
    </label>
  </div>

  <label class="field">
    <span class="caption">ラベルの文字</span>
    <textarea
      class="input"
      rows="6"
      placeholder="改行ごとに別のラベルになります"
      bind:value={text}
      disabled={printState === "printing"}></textarea>
  </label>

  <Settings
    bind:font
    bind:align
    bind:offsetPercent
    bind:fontScalePercent
    bind:marginMm
    disabled={printState === "printing"}
  />

  <div class="actions">
    <button
      class="btn primary"
      type="submit"
      disabled={printState === "printing" || !printable}
    >
      印刷
    </button>
    <span class="status" data-count={bodies.length}>{bodies.length} 枚</span>
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
  .continuous
    display: flex
    flex-direction: column
    gap: var(--sp-3)

  .header-row
    display: grid
    grid-template-columns: minmax(0, 2fr) minmax(0, 1fr)
    gap: var(--sp-3)

  .field
    display: flex
    flex-direction: column
    gap: var(--sp-1)
    min-width: 0

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

  .error-banner
    margin: 0
    padding: var(--sp-2)
    border-radius: var(--radius-sm)
    background: var(--c-danger-subtle)
    color: var(--c-danger)
    font-size: var(--fs-sm)
    white-space: pre-wrap
</style>
