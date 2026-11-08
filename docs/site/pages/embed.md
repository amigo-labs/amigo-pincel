> **Status:** the embeddable editor isn't published on npm yet. Follow along on
> [GitHub](https://github.com/amigo-labs/amigo-pincel); this page describes what
> the package will ship.

Pincel ships as `@amigo-labs/pincel`, a framework-agnostic npm package. Mount it
in any DOM element. Read and write Aseprite files. Listen for changes. Built on
`pincel-core` (Rust → WASM). Same editor, in your app, no iframe.

## Quick start

```bash
npm install @amigo-labs/pincel
```

```ts
import { Pincel, ColorMode } from '@amigo-labs/pincel';

const pincel = await Pincel.create({
  width: 64,
  height: 64,
  colorMode: ColorMode.Rgba,
});

pincel.mount(document.getElementById('editor'));

pincel.on('change', () => {
  console.log('document modified');
});

const bytes = await pincel.saveAseprite();
```

In a plain HTML page:

```html
<div id="editor" style="height: 480px"></div>
<script type="module">
  import { Pincel } from '@amigo-labs/pincel';
  const pincel = await Pincel.create({ width: 32, height: 32 });
  pincel.mount(document.getElementById('editor'));
</script>
```

## What you get

- Full editor UI, themeable via documented CSS variables.
- Public events: `change`, `save`, `selection`, `tool-change`, `palette-change`.
- Imperative API: `loadFile`, `saveAseprite`, `exportPng`, `setActiveLayer`, and more.
- TypeScript types shipped.
- Measured bundle size published per release.
- Works in any framework. React adapter, Svelte adapter, plain JS.

## What it costs

Free, MIT or Apache 2.0. No telemetry, no licensing, no per-seat fees. If you
ship a product using Pincel, you don't owe us anything. If you want to credit
us, that's nice.

## Honest limits

- The bundle is not tiny — it's an editor. For small embeds, consider a
  read-only viewer (Phase 2).
- No headless / Node API in Phase 1 (browser-only). Phase 2 adds Node for
  pipelines.
- Aseprite Lua scripts are not supported and not planned.

[Source on GitHub](https://github.com/amigo-labs/amigo-pincel)
