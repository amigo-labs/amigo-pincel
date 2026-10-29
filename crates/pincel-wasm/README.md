# @amigo-labs/pincel-wasm

WebAssembly bindings for the [Pincel] core: the pixel-art document model,
tools, command / undo stack and compositor, compiled from Rust with
[wasm-bindgen] and packaged by [wasm-pack] (`--target web`).

This is the low-level layer. It hands out a `Document` and an RGBA frame
buffer; drawing the frame, the canvas and every piece of UI is the
caller's job. For a mountable editor, use `@amigo-labs/pincel` instead.

## Install

```bash
npm install @amigo-labs/pincel-wasm
```

## Usage

```javascript
import init, { Document } from '@amigo-labs/pincel-wasm';

await init();

const doc = new Document(64, 64);          // or Document.openAseprite(bytes)
doc.applyTool('pencil', x, y, 0xff0000ff); // tool id, position, RGBA
const frame = doc.compose(0, 1);           // frame index, zoom
// frame.pixels is a Uint8ClampedArray view into wasm memory (zero-copy)

const bytes = doc.saveAseprite();
```

The document state lives in Rust and is never serialized into JS. Pull
changes with `doc.drainEvents()`, and release the handle with
`doc.free()` when you are done.

## Status

Phase 1. The API surface is documented in `docs/specs/pincel.md` §17.5 of
the [repository][Pincel] and kept small and stable; open work there is
tracked as feature specs indexed in `docs/specs/README.md`.

## License

Dual-licensed under MIT or Apache-2.0.

[Pincel]: https://github.com/amigo-labs/amigo-pincel
[wasm-bindgen]: https://rustwasm.github.io/wasm-bindgen/
[wasm-pack]: https://rustwasm.github.io/wasm-pack/
