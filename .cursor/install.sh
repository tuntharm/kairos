#!/usr/bin/env bash
# Idempotent Cloud Agent bootstrap for the Kairos monorepo.
# Prepares the Rust workspace (edition 2024), the Tauri Linux build
# dependencies, and the pnpm frontend workspace.
set -euo pipefail

echo "==> Installing Tauri v2 Linux system dependencies"
export DEBIAN_FRONTEND=noninteractive
sudo apt-get update -y
sudo apt-get install -y --no-install-recommends \
  build-essential \
  curl \
  wget \
  file \
  pkg-config \
  libssl-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libxdo-dev

echo "==> Ensuring a Rust toolchain with edition 2024 support (>= 1.85)"
if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile default
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
# The default image may ship an older toolchain; stable includes edition 2024.
rustup toolchain install stable --profile default
rustup default stable
rustup component add rustfmt clippy

echo "==> Installing pnpm workspace dependencies"
if ! command -v pnpm >/dev/null 2>&1; then
  corepack enable
  corepack prepare pnpm@10.0.0 --activate
fi
pnpm install --frozen-lockfile

echo "==> Bootstrap complete"
cargo --version
node --version
pnpm --version
