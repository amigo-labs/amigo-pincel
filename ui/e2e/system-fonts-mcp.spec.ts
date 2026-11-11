import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';
import {
  BUNDLED_FAMILY,
  bundledFamilyFaces,
  chooseFaces,
  faceFor,
  type FaceInfo,
} from '../src/lib/fonts/faces';

// docs/specs/system-fonts-mcp.md AC-05: the MCP tools choose a family's face
// for Bold / Italic as the editor does. pincel-agent carries its own copy of
// the face choice (it runs without the webview); this spec checks the
// editor's rules against the case table that pincel-agent's
// `face_choice_cases_match_the_shared_table` checks too. Node-side: no page.

const TABLE = fileURLToPath(
  new URL('../../crates/pincel-agent/src/face-choice-cases.json', import.meta.url),
);

interface CaseFace {
  readonly style: string;
  readonly weight: number;
  readonly italic: boolean;
  readonly stretch?: number;
}

interface Case {
  readonly name: string;
  readonly family: string;
  readonly faces: readonly CaseFace[];
  readonly bold: boolean;
  readonly italic: boolean;
  readonly want: { readonly style: string; readonly synth_bold: boolean; readonly synth_italic: boolean };
}

test('the editor chooses the faces of the shared face-choice table', async () => {
  const table = JSON.parse(await readFile(TABLE, 'utf-8')) as { cases: Case[] };
  expect(table.cases.length).toBeGreaterThanOrEqual(10);
  for (const c of table.cases) {
    const faces: FaceInfo[] = c.faces.map((f, i) => ({
      source: 'system',
      key: `case:${i}`,
      family: c.family,
      style: f.style,
      weight: f.weight,
      italic: f.italic,
      stretch: f.stretch ?? 5,
      postscript: `${c.family}-${f.style}`,
    }));
    const family = c.family === BUNDLED_FAMILY ? bundledFamilyFaces(faces) : chooseFaces(faces);
    if (family === null) throw new Error(`${c.name}: no faces`);
    const use = faceFor(family, c.bold, c.italic);
    const style = use.face.source === 'bundled' ? 'bundled' : use.face.style;
    expect({ style, synth_bold: use.synthBold, synth_italic: use.synthItalic }, c.name).toEqual(
      c.want,
    );
  }
});
