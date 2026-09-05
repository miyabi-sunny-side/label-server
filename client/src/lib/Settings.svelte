<script lang="ts">
  import Icon from "./Icon.svelte";
  import { fetchFonts, type Align } from "./api";

  /**
   * The settings both print modes share, bound by the page that owns
   * them. 文字サイズ is the one that gets touched every print, so it
   * stays out in the open; the rest sit behind 詳細, closed by default.
   * The values live in the page, not in this markup, so a closed
   * accordion still sends every one of them.
   */
  let {
    font = $bindable(),
    align = $bindable(),
    offsetPercent = $bindable(),
    fontScalePercent = $bindable(),
    marginMm = $bindable(),
    disabled = false,
  }: {
    font: string | null;
    align: Align;
    offsetPercent: number;
    fontScalePercent: number;
    marginMm: number;
    disabled?: boolean;
  } = $props();

  let fonts = $state<string[]>([]);
  let detailsOpen = $state(false);

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
</script>

<div class="settings">
  <label class="field size">
    <span class="caption">文字サイズ (%)</span>
    <input
      class="input"
      type="number"
      min="10"
      max="100"
      step="10"
      bind:value={fontScalePercent}
      {disabled}
    />
  </label>
  <button
    class="btn details-toggle"
    type="button"
    aria-expanded={detailsOpen}
    aria-controls="settings-details"
    onclick={() => (detailsOpen = !detailsOpen)}
  >
    詳細
    <span class="chevron" class:open={detailsOpen} aria-hidden="true">
      <Icon name="chevron-down" />
    </span>
  </button>
</div>

{#if detailsOpen}
  <div class="details" id="settings-details">
    <label class="field">
      <span class="caption">フォント</span>
      <select class="input" bind:value={font} {disabled}>
        {#each fonts as id (id)}
          <option value={id}>{id}</option>
        {/each}
      </select>
    </label>
    <label class="field">
      <span class="caption">揃え</span>
      <select class="input" bind:value={align} {disabled}>
        <option value="left">左寄せ</option>
        <option value="center">中央寄せ</option>
        <option value="right">右寄せ</option>
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
        {disabled}
      />
    </label>
    <label class="field">
      <span class="caption">余白 (mm)</span>
      <input
        class="input"
        type="number"
        min="2"
        max="127"
        bind:value={marginMm}
        {disabled}
      />
    </label>
  </div>
{/if}

<style lang="sass">
  .settings
    display: flex
    align-items: flex-end
    gap: var(--sp-3)

  .size
    max-width: 160px

  .details-toggle
    display: flex
    align-items: center
    gap: var(--sp-1)

  .chevron
    display: flex

    &.open
      transform: rotate(180deg)

  .details
    display: grid
    grid-template-columns: minmax(0, 2fr) repeat(3, minmax(0, 1fr))
    gap: var(--sp-3)
    padding: var(--sp-3)
    border: 1px solid var(--c-border)
    border-radius: var(--radius-sm)
    background: var(--c-surface-raised)

  @media (max-width: 767px)
    .details
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr)

      .field:first-child
        grid-column: 1 / -1

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
</style>
