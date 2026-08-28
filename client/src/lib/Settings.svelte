<script lang="ts">
  import { fetchFonts, type Align } from "./api";

  /** The settings both print modes share, bound by the page that owns them. */
  let {
    font = $bindable(),
    align = $bindable(),
    offsetPercent = $bindable(),
    fontScalePercent = $bindable(),
    disabled = false,
  }: {
    font: string | null;
    align: Align;
    offsetPercent: number;
    fontScalePercent: number;
    disabled?: boolean;
  } = $props();

  let fonts = $state<string[]>([]);

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
    <span class="caption">文字サイズ (%)</span>
    <input
      class="input"
      type="number"
      min="10"
      max="100"
      bind:value={fontScalePercent}
      {disabled}
    />
  </label>
</div>

<style lang="sass">
  .settings
    display: grid
    grid-template-columns: minmax(0, 2fr) repeat(3, minmax(0, 1fr))
    gap: var(--sp-3)

  @media (max-width: 767px)
    .settings
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
