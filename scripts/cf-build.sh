#!/usr/bin/env bash
#
# Cloudflare Workers Builds — build the marketing site AND the editor into one
# static-assets directory, `website/build/`, served by the `amigo-pincel`
# Worker (root wrangler.toml). The site owns `/`, the editor `/app/`.
# docs/specs/pincel.md §15, 2026-10-12.
#
# Cloudflare's build image ships Node but not the Rust / wasm-pack toolchain
# the editor's wasm cores need, so this script installs what is missing first.
# It is also safe to run locally: every install step is skipped when the tool
# is already on PATH.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"

# --- Rust toolchain -------------------------------------------------------
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown

# --- wasm-pack (prebuilt if possible, else from source) -------------------
if ! command -v wasm-pack >/dev/null 2>&1; then
  curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh || true
fi
if ! command -v wasm-pack >/dev/null 2>&1; then
  cargo install wasm-pack --locked
fi

# --- wasm-opt (binaryen) ---------------------------------------------------
# Both wasm crates switch wasm-pack's own wasm-opt pass off, and Cloudflare's
# image has no binaryen: fetch a pinned release build (a static binary).
# The release workflow gets the same tool from apt.
BINARYEN_VERSION=133
if ! command -v wasm-opt >/dev/null 2>&1; then
  binaryen_dir="$HOME/.local/binaryen-$BINARYEN_VERSION"
  mkdir -p "$binaryen_dir"
  curl --proto '=https' --tlsv1.2 -sSfL \
    "https://github.com/WebAssembly/binaryen/releases/download/version_$BINARYEN_VERSION/binaryen-version_$BINARYEN_VERSION-x86_64-linux.tar.gz" |
    tar -xzf - -C "$binaryen_dir" --strip-components=2 "binaryen-version_$BINARYEN_VERSION/bin/wasm-opt"
  export PATH="$binaryen_dir:$PATH"
fi
wasm-opt --version

# --- pnpm -----------------------------------------------------------------
if ! command -v pnpm >/dev/null 2>&1; then
  corepack enable
fi

# --- Marketing site → website/build/ --------------------------------------
cd "$root/website"
pnpm install --frozen-lockfile
pnpm build

# --- Editor → ui/dist/app/ ------------------------------------------------
# The wasm packages must exist before `pnpm install` (ui preinstall check).
# wasm-pack's own wasm-opt pass is off in both crates' Cargo.toml, so the
# wasm-opt installed above runs here; the check below still fails the deploy
# on a module that skipped it (docs/specs/performance-build-simd.md AC-03).
cd "$root/ui"
pnpm wasm:build:release
# Both modules ship with wasm SIMD (docs/specs/performance-build-simd.md AC-02).
node "$root/scripts/check-wasm-simd.mjs" ../crates/pincel-wasm/pkg ../crates/pincel-image-wasm/pkg
for wasm in ../crates/pincel-wasm/pkg/pincel_wasm_bg.wasm   ../crates/pincel-image-wasm/pkg/pincel_image_wasm_bg.wasm; do
  wasm-opt -O3 --enable-simd -o "$wasm" "$wasm"
done
pnpm install --frozen-lockfile
pnpm build
node "$root/scripts/check-wasm-release.mjs" "$root/ui/dist/app/assets"

# --- Merge: the editor lands under the site's /app/ -----------------------
rm -rf "$root/website/build/app"
cp -R "$root/ui/dist/app" "$root/website/build/app"

echo "Built site into website/build/ and the editor into website/build/app/"
