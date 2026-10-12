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
# wasm-pack's own wasm-opt pass is off in both crates' Cargo.toml; run a
# system wasm-opt when the image has one, else ship the rustc release build.
cd "$root/ui"
pnpm wasm:build:release
if command -v wasm-opt >/dev/null 2>&1; then
  for wasm in ../crates/pincel-wasm/pkg/pincel_wasm_bg.wasm \
    ../crates/pincel-image-wasm/pkg/pincel_image_wasm_bg.wasm; do
    wasm-opt -O3 -o "$wasm" "$wasm"
  done
fi
pnpm install --frozen-lockfile
pnpm build

# --- Merge: the editor lands under the site's /app/ -----------------------
rm -rf "$root/website/build/app"
cp -R "$root/ui/dist/app" "$root/website/build/app"

echo "Built site into website/build/ and the editor into website/build/app/"
