#!/bin/bash
set -e

# The target/ named volume is created root-owned by docker, so hand it to the
# container user before cargo touches it. Without this, cargo fails with
# "failed to create file ... Permission denied".
echo "Claiming the build directory..."
sudo chown -R "$(id -u):$(id -g)" target

echo "Installing system dependencies..."
sudo apt-get update && sudo apt-get install -y sqlite3 perl

echo "Updating Rust toolchain..."
rustup update stable
rustup component add clippy rustfmt

echo "Installing diesel_cli (using bundled sqlite)..."
cargo install diesel_cli --no-default-features --features sqlite-bundled

echo "Installing frontend dependencies..."
cd ui && npm ci && cd ..

# dist/ is gitignored, and the backend serves it directly, so a fresh clone has
# no UI to serve until this runs.
echo "Building the UI..."
cd ui && npm run build && cd ..

echo "Building Rust project..."
cargo build

echo "Dev container setup complete!"
echo ""
echo "To run the app:"
echo "  cargo run -p twitch-points-miner -- -t data/tokens.json --analytics-db data/analytics.db"
echo ""
echo "To run frontend dev server:"
echo "  cd ui && npm run dev"
