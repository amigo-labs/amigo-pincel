<script lang="ts">
  import '$lib/styles/app.css';
  import Header from '$lib/components/Header.svelte';
  import Footer from '$lib/components/Footer.svelte';
  import { crt } from '$lib/stores/crt.svelte';
  import { onMount } from 'svelte';
  import { page } from '$app/state';

  const { children } = $props();

  onMount(() => {
    crt.hydrate();
  });

  const hideChrome = $derived(page.url.pathname.startsWith('/app'));
</script>

<div class="root" class:crt-enabled={crt.enabled}>
  {#if !hideChrome}
    <Header />
  {/if}
  <main>
    {@render children?.()}
  </main>
  {#if !hideChrome}
    <Footer />
  {/if}
</div>

<style>
  .root {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
  }
  main {
    flex: 1;
  }
</style>
