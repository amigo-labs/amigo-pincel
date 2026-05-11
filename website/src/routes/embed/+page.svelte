<script lang="ts">
  import SeoHead from '$lib/components/SeoHead.svelte';
  import PixelIcon from '$lib/components/PixelIcon.svelte';

  const install = `npm install @amigo-labs/pincel`;

  const usage = `import { Pincel, ColorMode } from '@amigo-labs/pincel';

const pincel = await Pincel.create({
  width: 64,
  height: 64,
  colorMode: ColorMode.Rgba,
});

pincel.mount(document.getElementById('editor'));

pincel.on('change', () => {
  console.log('document modified');
});

const bytes = await pincel.saveAseprite();`;
</script>

<SeoHead
  title="Embed Pincel — For developers"
  description="@amigo-labs/pincel is a framework-agnostic npm package. Drop a full pixel-art editor into your tool with one mount call. Read and write Aseprite files."
/>

<div class="page">
  <header class="page-head">
    <p class="caption">For tool makers</p>
    <h1 class="h-display">Embed Pincel in your tool.</h1>
    <p class="lede">
      Pincel ships as <code>@amigo-labs/pincel</code>, a framework-agnostic npm package. Mount it
      in any DOM element. Read and write Aseprite files. Listen for changes. Built on
      <code>pincel-core</code> (Rust → WASM). Same editor, in your app, no iframe.
    </p>
  </header>

  <section class="block">
    <h2>Quick start</h2>
    <pre><code>{install}</code></pre>
    <pre><code>{usage}</code></pre>
  </section>

  <section class="block">
    <h2>Live demo</h2>
    <div class="demo">
      <div class="demo-frame" aria-label="Embedded Pincel demo">
        <div class="demo-placeholder">
          <PixelIcon name="plug" size={64} />
          <p>Live embed loads here at launch.</p>
          <p class="muted">
            For now, try the full editor at
            <a href="/app">/app</a>.
          </p>
        </div>
      </div>
      <details class="source">
        <summary>View source</summary>
        <pre><code>{`<div id="editor" style="height: 480px"></div>
<script type="module">
  import { Pincel } from '@amigo-labs/pincel';
  const pincel = await Pincel.create({ width: 32, height: 32 });
  pincel.mount(document.getElementById('editor'));
</` + `script>`}</code></pre>
      </details>
    </div>
  </section>

  <section class="block">
    <h2>What you get</h2>
    <ul class="bullets">
      <li>Full editor UI, themeable via documented CSS variables.</li>
      <li>
        Public events: <code>change</code>, <code>save</code>, <code>selection</code>,
        <code>tool-change</code>, <code>palette-change</code>.
      </li>
      <li>
        Imperative API: <code>loadFile</code>, <code>saveAseprite</code>, <code>exportPng</code>,
        <code>setActiveLayer</code>, and more.
      </li>
      <li>TypeScript types shipped.</li>
      <li>Measured bundle size published per release.</li>
      <li>Works in any framework. React adapter, Svelte adapter, plain JS.</li>
    </ul>
  </section>

  <section class="block">
    <h2>What it costs</h2>
    <p>
      Free, MIT or Apache 2.0. No telemetry, no licensing, no per-seat fees. If you ship a product
      using Pincel, you don't owe us anything. If you want to credit us, that's nice.
    </p>
  </section>

  <section class="block">
    <h2>Honest limits</h2>
    <ul class="bullets">
      <li>
        Bundle is not tiny — it's an editor. For small embeds, consider a read-only viewer
        (Phase 2).
      </li>
      <li>No headless / Node API in Phase 1 (browser-only). Phase 2 adds Node for pipelines.</li>
      <li>Aseprite Lua scripts are not supported and not planned.</li>
    </ul>
  </section>

  <section class="cta-block">
    <a class="btn-pixel" href="https://github.com/amigo-labs/amigo-pincel">
      <PixelIcon name="github" size={16} />
      Source on GitHub
    </a>
    <a class="btn-pixel btn-pixel--secondary" href="/app">
      Try the editor first
      <PixelIcon name="arrow-right" size={16} />
    </a>
  </section>
</div>

<style>
  .page {
    max-width: 900px;
    margin: 0 auto;
    padding: 4rem 1.5rem 2rem;
  }
  .page-head {
    margin-bottom: 3rem;
  }
  h1 {
    font-size: 3rem;
    line-height: 1.05;
    margin: 0.5rem 0 1rem;
    color: var(--color-fg-primary);
  }
  .lede {
    color: var(--color-fg-secondary);
    font-size: 1.1rem;
    line-height: 1.65;
    margin: 0;
    max-width: 64ch;
  }
  .lede code,
  .block code {
    color: var(--color-brand-primary);
  }
  .block {
    margin: 0 0 3rem;
  }
  .block h2 {
    font-family: var(--font-pixel);
    font-size: 1.85rem;
    line-height: 1;
    margin: 0 0 1rem;
    color: var(--color-fg-primary);
  }
  .block p,
  .bullets li {
    color: var(--color-fg-secondary);
    line-height: 1.65;
  }
  .bullets {
    margin: 0;
    padding: 0 0 0 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  pre {
    margin: 0 0 1rem;
  }
  .demo {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .demo-frame {
    width: 100%;
    aspect-ratio: 16 / 9;
    background-color: var(--color-bg-card);
    border: 2px solid var(--color-bg-base);
    box-shadow: 4px 4px 0 var(--color-bg-base), 0 0 0 1px var(--color-border-strong);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .demo-placeholder {
    text-align: center;
    color: var(--color-fg-muted);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
  }
  .demo-placeholder p {
    margin: 0;
    color: var(--color-fg-secondary);
  }
  .demo-placeholder .muted {
    color: var(--color-fg-muted);
    font-size: 0.9rem;
  }
  .demo-placeholder a {
    color: var(--color-brand-primary);
  }
  summary {
    cursor: pointer;
    color: var(--color-fg-secondary);
    font-family: var(--font-mono);
    font-size: 0.85rem;
    padding: 0.5rem 0;
  }
  summary:hover {
    color: var(--color-fg-primary);
  }
  .cta-block {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    margin-top: 4rem;
  }
  @media (max-width: 720px) {
    h1 {
      font-size: 2.25rem;
    }
    .page {
      padding: 2rem 1rem;
    }
  }
</style>
