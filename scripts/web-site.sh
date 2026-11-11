#!/usr/bin/env bash
#
# Turn the web build in ui/dist/ (`pnpm build` in ui/, the editor at the
# root) into the site the `amigo-pincel` Worker serves at
# pincel.amigo-labs.dev (root wrangler.toml): add the OCR models and the
# files in cloudflare/ (redirects, headers, 404 page, the stand-in worker
# at /app/sw.js). release.yml runs it before it zips the site as
# pincel-web.zip and deploys that zip (docs/specs/pincel.md §15,
# 2026-11-11; docs/RELEASING.md "Web deploy").
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dist="$root/ui/dist"
test -f "$dist/index.html" || { echo "no web build in ui/dist (run pnpm build in ui/)" >&2; exit 1; }

# --- OCR models → ui/dist/ocr/tessdata_fast-<version>/ ----------------------
# Tools ▸ Recognize Text downloads a language's model on first use from this
# path (docs/specs/ocr-dialog.md; Decision Log 2026-11-08). The files are the
# ones committed with pincel-ocr; the UI pins their SHA-256
# (ui/src/modes/image/ocr/catalog.ts). They are added after the build, so the
# PWA's precache never lists them.
OCR_VERSION=4.1.0
ocr_dir="$dist/ocr/tessdata_fast-$OCR_VERSION"
mkdir -p "$ocr_dir"
cp "$root/crates/pincel-ocr/models/deu.traineddata" "$root/crates/pincel-ocr/models/eng.traineddata" "$ocr_dir/"

# --- Root files: redirects, headers, 404 page, /app/sw.js -----------------
cp -R "$root/cloudflare/." "$dist/"

echo "Added the OCR models (ui/dist/ocr/) and cloudflare/ to the web build in ui/dist/"
