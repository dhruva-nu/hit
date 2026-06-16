#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Building hitpoint..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

DEST="$HOME/.local/bin"
mkdir -p "$DEST"

cp "$SCRIPT_DIR/target/release/hitpoint" "$DEST/hitpoint"
ln -sf "$DEST/hitpoint" "$DEST/hit"

echo "Installed: $DEST/hit (and $DEST/hitpoint)"

if ! echo "$PATH" | grep -q "$DEST"; then
    echo ""
    echo "Note: $DEST is not in your PATH. Add this to your shell config:"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
