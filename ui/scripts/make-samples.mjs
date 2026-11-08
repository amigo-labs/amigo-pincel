// Rebuilds the start screen's sample documents (docs/specs/first-run-tutorial.md
// AC-13 – AC-16) through the pincel-mcp server, so they are reproducible:
//   cargo build -p pincel-mcp && node ui/scripts/make-samples.mjs
// Writes ui/public/samples/slime.aseprite and postcard.ora (CC0).
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '../..');
const server = spawn(join(root, 'target/debug/pincel-mcp'), [
  '--workspace',
  join(root, 'ui/public/samples'),
]);
const lines = createInterface({ input: server.stdout });
const waiting = new Map();
lines.on('line', (line) => {
  const msg = JSON.parse(line);
  waiting.get(msg.id)?.(msg);
  waiting.delete(msg.id);
});
let next = 1;
function rpc(method, params) {
  const id = next++;
  server.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  return new Promise((resolve) => waiting.set(id, resolve));
}
async function call(name, args) {
  const msg = await rpc('tools/call', { name, arguments: args });
  const result = msg.result;
  const text = result?.content?.[0]?.text ?? '';
  if (msg.error || result?.isError) throw new Error(`${name}: ${JSON.stringify(msg.error ?? text)}`);
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

await rpc('initialize', {
  protocolVersion: '2025-06-18',
  capabilities: {},
  clientInfo: { name: 'make-samples', version: '1' },
});

// A bouncing slime: 32 × 32, two layers, four frames, a tag and a slice.
await call('doc.new', { mode: 'pixel', width: 32, height: 32 });
const body = (await call('layer.list', {})).layers[0].id;
await call('layer.set', { layer: body, name: 'Body' });
const eyes = (await call('layer.add', { name: 'Eyes' })).id ?? (await call('layer.list', {})).layers.at(-1).id;
for (let f = 1; f < 4; f += 1) await call('frame.add', { duration_ms: 120 });
const squash = [0, 2, 3, 1];
for (let f = 0; f < 4; f += 1) {
  const top = 10 + squash[f];
  await call('draw.ellipse', { layer: body, frame: f, x0: 5, y0: top, x1: 26, y1: 27, color: '#3fbf5f', filled: true });
  await call('draw.ellipse', { layer: body, frame: f, x0: 5, y0: top, x1: 26, y1: 27, color: '#1f6f3f' });
  await call('draw.rect', { layer: eyes, frame: f, x0: 11, y0: top + 6, x1: 12, y1: top + 8, color: '#ffffff', filled: true });
  await call('draw.rect', { layer: eyes, frame: f, x0: 19, y0: top + 6, x1: 20, y1: top + 8, color: '#ffffff', filled: true });
}
await call('tag.add', { name: 'bounce', from: 0, to: 3 });
await call('slice.add', { name: 'hitbox', bounds: { x: 5, y: 10, width: 22, height: 18 } });
await call('doc.save', { path: 'slime.aseprite' });

// A postcard: 320 × 200, three layers (sky, sun, greeting).
await call('doc.new', { mode: 'image', width: 320, height: 200 });
await call('draw.gradient', { x0: 0, y0: 0, x1: 0, y1: 200, from: '#6fb7ff', to: '#ffe7b0' });
await call('layer.add', {});
await call('draw.shape', { shape: 'ellipse', x0: 220, y0: 30, x1: 290, y1: 100, fill: '#ffcf3f' });
await call('layer.add', {});
await call('draw.text', { text: 'Hello from Pincel', x: 24, y: 140, size: 28, color: '#2f2f4f' });
await call('doc.save', { path: 'postcard.ora' });

server.stdin.end();
