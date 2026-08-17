#!/bin/bash
set -e

echo "Installing system dependencies..."
sudo apt-get update && sudo apt-get install -y sqlite3 perl

echo "Updating Rust toolchain..."
rustup update stable
rustup component add clippy rustfmt

echo "Installing diesel_cli (using bundled sqlite)..."
cargo install diesel_cli --no-default-features --features sqlite-bundled

echo "Installing frontend dependencies..."
cd frontend && bun install && bun x update-browserslist-db@latest --yes && cd ..

echo "Building Rust project..."
cargo build

echo "Dev container setup complete!"
echo ""
echo "To run the app:"
echo "  cargo run -p twitch-points-miner -- -t data/tokens.json --analytics-db data/analytics.db"
echo ""
echo "To run frontend dev server:"
echo "  cd frontend && bun run dev"
