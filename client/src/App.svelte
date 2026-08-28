<script lang="ts">
  import Header from "./lib/Header.svelte";
  import { initRouter, router } from "./lib/router.svelte";
  import Continuous from "./pages/Continuous.svelte";
  import Print from "./pages/Print.svelte";

  $effect(() => initRouter());
</script>

<svelte:head>
  <title>label-server</title>
</svelte:head>

<Header />

<main>
  {#if router.index === 1}
    <Continuous />
  {:else}
    <Print />
  {/if}
</main>
