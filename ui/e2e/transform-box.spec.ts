import { expect, test } from '@playwright/test';
import {
  applyMatrix,
  boxOfBounds,
  corners,
  dragCorner,
  dragEdge,
  handles,
  hitBox,
  movePivot,
  patchMatrix,
  pivotPoint,
  relativeMatrix,
  rotateBox,
  translateBox,
  type BoxState,
} from '../src/modes/image/tools/transform-box';

// The transform box's geometry (docs/specs/image-transform-handles.md),
// Node only: what each drag does to the box the core then applies.

const box = boxOfBounds([10, 20, 40, 20]);
const close = (a: readonly number[], b: readonly number[]): void => {
  expect(a.length).toBe(b.length);
  a.forEach((v, i) => expect(v).toBeCloseTo(b[i] ?? NaN, 6));
};

test('a corner drag keeps the opposite corner and scales along the box axes (AC-03)', () => {
  // Bottom-right corner (2) to (70, 60): top-left stays at (10, 20).
  const b = dragCorner(box, 2, [70, 60], false);
  close(corners(b)[0] ?? [], [10, 20]);
  close(b.size, [60, 40]);
  // Turned 90°: the box's own axes, not the canvas'.
  const turned: BoxState = { ...box, angle: 90 };
  const fixed = corners(turned)[0] ?? [0, 0];
  const t = dragCorner(turned, 2, [fixed[0] - 30, fixed[1] + 60], false);
  close(corners(t)[0] ?? [], fixed);
  close(t.size, [60, 30]);
});

test('a box never flips and never gets below 1 px (AC-03)', () => {
  const b = dragCorner(box, 2, [0, 0], false);
  close(b.size, [1, 1]);
  close(corners(b)[0] ?? [], [10, 20]);
  const e = dragEdge(box, 'e', [-100, 30]);
  close(e.size, [1, 20]);
});

test('Shift keeps the starting width-to-height ratio on a corner drag (AC-04)', () => {
  const b = dragCorner(box, 2, [130, 25], true);
  expect(b.size[0] / b.size[1]).toBeCloseTo(2, 6);
  close(corners(b)[0] ?? [], [10, 20]);
});

test('an edge drag scales along its axis only, the opposite edge fixed (AC-03)', () => {
  const b = dragEdge(box, 'n', [30, 0]);
  close(b.size, [40, 40]);
  close(corners(b)[3] ?? [], [10, 40]);
  close(corners(b)[2] ?? [], [50, 40]);
});

test('the pivot keeps its place relative to the box when it scales or moves (AC-02, AC-03)', () => {
  const p = movePivot(box, [20, 25]);
  close(p.pivot, [0.25, 0.25]);
  const scaled = dragCorner(p, 2, [90, 60], false);
  close(scaled.pivot, [0.25, 0.25]);
  close(pivotPoint(scaled), [10 + 0.25 * 80, 20 + 0.25 * 40]);
  close(pivotPoint(translateBox(p, 5, -3)), [25, 22]);
});

test('a rotation turns about the pivot, clockwise positive; Shift snaps to 15° (AC-05, AC-06)', () => {
  const center = pivotPoint(box);
  // A quarter turn swept from east to south: +90°.
  const r = rotateBox(box, [center[0] + 10, center[1]], [center[0], center[1] + 10], false);
  expect(r.angle).toBeCloseTo(90, 6);
  close(r.center, box.center);
  // About a moved pivot the centre swings round it.
  const p = movePivot(box, [10, 20]);
  const r2 = rotateBox(p, [20, 20], [10, 30], false);
  expect(r2.angle).toBeCloseTo(90, 6);
  close(pivotPoint(r2), [10, 20]);
  // Shift: 20° snaps to 15°.
  const a = (20 * Math.PI) / 180;
  const s = rotateBox(
    box,
    [center[0] + 10, center[1]],
    [center[0] + 10 * Math.cos(a), center[1] + 10 * Math.sin(a)],
    true,
  );
  expect(s.angle).toBeCloseTo(15, 6);
});

test('hit-testing in screen pixels: pivot, handles, inside, outside (AC-05)', () => {
  expect(hitBox(box, [30, 30], 1).kind).toBe('pivot');
  expect(hitBox(box, [10, 20], 1)).toEqual({ kind: 'corner', corner: 0 });
  expect(hitBox(box, [50, 30], 1)).toEqual({ kind: 'edge', edge: 'e' });
  // At zoom 4 the 12 px handles cover less of the canvas.
  expect(hitBox(box, [20, 35], 4).kind).toBe('inside');
  // 12 screen px around the outline still count as the box…
  expect(hitBox(box, [40, 50], 1).kind).toBe('inside');
  // …farther out is outside, and the radius shrinks in canvas px at zoom.
  expect(hitBox(box, [40, 55], 1).kind).toBe('outside');
  expect(hitBox(box, [40, 45], 4).kind).toBe('outside');
  expect(handles(box)).toHaveLength(8);
});

test('the preview maps: the patch onto the box, one box onto another (AC-20, AC-23)', () => {
  const bounds = [10, 20, 40, 20] as const;
  // Scaled, turned and moved: the patch's corners land on the box's.
  const b = rotateBox(dragCorner(box, 2, [90, 50], false), [70, 20], [60, 70], false);
  const m = patchMatrix(b, bounds);
  const patchCorners: [number, number][] = [
    [0, 0],
    [40, 0],
    [40, 20],
    [0, 20],
  ];
  patchCorners.forEach((p, i) => close(applyMatrix(m, p), corners(b)[i] ?? []));
  // The untouched box maps patch pixels to where they are.
  close(applyMatrix(patchMatrix(box, bounds), [5, 7]), [15, 27]);
  // From the box at the start of a drag to the box now.
  const r = relativeMatrix(box, b);
  expect(r).not.toBeNull();
  corners(box).forEach((p, i) =>
    close(applyMatrix(r ?? [1, 0, 0, 0, 1, 0], p), corners(b)[i] ?? []),
  );
});
