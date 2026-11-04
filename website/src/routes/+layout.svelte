<script lang="ts">
  import '#lib/styles/app.css';
  import Header from '#lib/components/Header.svelte';
  import Footer from '#lib/components/Footer.svelte';
  import { crt } from '#lib/stores/crt.svelte.ts';
  import { lang } from '#lib/i18n/index.ts';
  import { onMount } from 'svelte';

  const { children } = $props();

  onMount(() => {
    crt.hydrate();
  });

  // hooks.server.ts fills `<html lang>` for the prerendered page; a
  // client-side navigation (the language switch) keeps the old document,
  // so follow the route's language here (website spec §11).
  $effect(() => {
    document.documentElement.lang = lang();
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
