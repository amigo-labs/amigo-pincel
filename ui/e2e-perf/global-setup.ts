import { rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** Starts every performance run with an empty results file. */
export default function globalSetup(): void {
  rmSync(fileURLToPath(new URL('./results/browser.json', import.meta.url)), { force: true });
}
