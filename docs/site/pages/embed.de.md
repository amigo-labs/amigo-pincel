> **Stand:** Der einbettbare Editor ist noch nicht auf npm veröffentlicht.
> Verfolge den Stand auf [GitHub](https://github.com/amigo-labs/amigo-pincel);
> diese Seite beschreibt, was das Paket bringen wird.

Pincel gibt es als `@amigo-labs/pincel`, ein Framework-unabhängiges npm-Paket. In
jedes DOM-Element einhängen. Aseprite-Dateien lesen und schreiben. Auf Änderungen
hören. Gebaut auf `pincel-core` (Rust → WASM). Derselbe Editor, in deiner App,
ohne iframe.

## Schnellstart

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

In einer einfachen HTML-Seite:

```html
<div id="editor" style="height: 480px"></div>
<script type="module">
  import { Pincel } from '@amigo-labs/pincel';
  const pincel = await Pincel.create({ width: 32, height: 32 });
  pincel.mount(document.getElementById('editor'));
</script>
```

## Was du bekommst

- Vollständige Editor-Oberfläche, gestaltbar über dokumentierte CSS-Variablen.
- Öffentliche Events: `change`, `save`, `selection`, `tool-change`, `palette-change`.
- Imperative API: `loadFile`, `saveAseprite`, `exportPng`, `setActiveLayer` und mehr.
- TypeScript-Typen inklusive.
- Gemessene Bundle-Größe zu jedem Release.
- Funktioniert in jedem Framework. Adapter für React und Svelte, oder reines JS.

## Was es kostet

Kostenlos, MIT oder Apache 2.0. Keine Telemetrie, keine Lizenzen, keine Gebühren
pro Platz. Wenn du ein Produkt mit Pincel auslieferst, schuldest du uns nichts.
Eine Erwähnung freut uns trotzdem.

## Ehrliche Grenzen

- Das Bundle ist nicht winzig – es ist ein Editor. Für kleine Einbettungen kommt
  ein reiner Betrachter (Phase 2).
- Keine Node-API ohne Oberfläche in Phase 1 (nur Browser). Phase 2 bringt Node
  für Pipelines.
- Lua-Skripte von Aseprite werden nicht unterstützt und sind nicht geplant.

[Quelltext auf GitHub](https://github.com/amigo-labs/amigo-pincel)
