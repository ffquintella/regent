#!/usr/bin/env bash
# Update Homebrew Formula with SHA256 checksums
# Run this after uploading release artifacts to GitHub

set -euo pipefail

VERSION="${1:-}"
RELEASE_DIR="${2:-target/homebrew-release}"

if [[ -z "$VERSION" ]]; then
  echo "Usage: $0 <version> [release-dir]"
  echo "Example: $0 0.1.1 target/homebrew-release"
  exit 1
fi

FORMULA="packaging/homebrew/regent.rb"

echo "🔐 Updating SHA256 checksums in $FORMULA..."

# Read checksums
declare -A checksums
for target in x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  sha_file="$RELEASE_DIR/regent-$VERSION-$target.tar.gz.sha256"
  if [[ -f "$sha_file" ]]; then
    checksums[$target]=$(awk '{print $1}' "$sha_file")
    echo "  $target: ${checksums[$target]}"
  else
    echo "  ⚠️  Warning: $sha_file not found"
  fi
done

# Update formula
# Note: This is a simple implementation. For production, consider using a template engine
echo ""
echo "✏️  Manual steps required:"
echo "  1. Edit $FORMULA"
echo "  2. Replace SHA256 values with:"
echo ""
for target in "${!checksums[@]}"; do
  echo "     $target: ${checksums[$target]}"
done
echo ""
echo "  3. Verify URLs point to correct GitHub release"
echo "  4. Test with: brew install --build-from-source ./$FORMULA"
