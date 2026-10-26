<script lang="ts">
  import '#lib/styles/app.css';
  import Header from '#lib/components/Header.svelte';
  import Footer from '#lib/components/Footer.svelte';
  import { crt } from '#lib/stores/crt.svelte.ts';
  import { onMount } from 'svelte';

  const { children } = $props();

  onMount(() => {
    crt.hydrate();
  });
</script>

<div class="root" class:crt-enabled={crt.enabled}>
  <Header />
  <main>
    {@render children?.()}
  </main>
  <Footer />
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
