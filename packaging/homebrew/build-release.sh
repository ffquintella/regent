#!/usr/bin/env bash
# Homebrew Release Script
# This script builds release binaries and updates the Homebrew formula

set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.1.1"
  exit 1
fi

echo "🍺 Building Regent v$VERSION for Homebrew..."

# Build directory
BUILD_DIR="target/homebrew-release"
mkdir -p "$BUILD_DIR"

# Targets to build
TARGETS=(
  "x86_64-apple-darwin"
  "aarch64-apple-darwin"
  "x86_64-unknown-linux-gnu"
  "aarch64-unknown-linux-gnu"
)

echo "📦 Building release binaries..."
for target in "${TARGETS[@]}"; do
  echo "  Building for $target..."
  
  # Build the binary
  cargo build --release --target "$target"
  
  # Create archive directory
  archive_dir="$BUILD_DIR/regent-$VERSION-$target"
  mkdir -p "$archive_dir"
  
  # Copy binary
  if [[ "$target" == *"darwin"* ]] || [[ "$target" == *"linux"* ]]; then
    cp "target/$target/release/regent" "$archive_dir/"
  fi
  
  # Create tarball
  tar -czf "$BUILD_DIR/regent-$VERSION-$target.tar.gz" -C "$BUILD_DIR" "regent-$VERSION-$target"
  
  # Generate SHA256
  sha256sum "$BUILD_DIR/regent-$VERSION-$target.tar.gz" > "$BUILD_DIR/regent-$VERSION-$target.tar.gz.sha256"
  
  echo "  ✓ Created $BUILD_DIR/regent-$VERSION-$target.tar.gz"
done

echo ""
echo "✨ Release artifacts created in $BUILD_DIR"
echo ""
echo "📝 Next steps:"
echo "  1. Upload tarballs to GitHub releases"
echo "  2. Update SHA256 checksums in regent.rb formula"
echo "  3. Test installation with: brew install --build-from-source ./packaging/homebrew/regent.rb"
echo "  4. Submit to homebrew-core or create a tap"
