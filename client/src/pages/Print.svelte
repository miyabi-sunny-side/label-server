<script lang="ts">
  import { PrintError, postPrint } from "../lib/api";

  type PrintState = "idle" | "printing" | "success" | "error";

  let text = $state("");
  let printState = $state<PrintState>("idle");
  let errorMessage = $state("");
  const printable = $derived(text.trim().length > 0);

  async function print() {
    if (!printable || printState === "printing") {
      return;
    }
    printState = "printing";
    try {
      await postPrint(text);
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
    resize: vertical

    &:focus
      border-color: var(--c-accent)

    &:disabled
      opacity: 0.5

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
