#!/usr/bin/env bash
# Generate Debian package for Regent
# Requires: dpkg-deb, fakeroot

set -euo pipefail

VERSION="${1:-0.1.1}"
ARCH="${2:-amd64}"  # amd64 or arm64
BUILD_DIR="target/debian-$ARCH"

echo "📦 Building Debian package for regent v$VERSION ($ARCH)..."

# Clean and create build directory
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/DEBIAN"
mkdir -p "$BUILD_DIR/usr/bin"
mkdir -p "$BUILD_DIR/usr/share/man/man1"
mkdir -p "$BUILD_DIR/usr/share/bash-completion/completions"
mkdir -p "$BUILD_DIR/usr/share/zsh/vendor-completions"
mkdir -p "$BUILD_DIR/usr/share/fish/vendor_completions.d"

# Map Rust targets to Debian architectures
case "$ARCH" in
  amd64)
    RUST_TARGET="x86_64-unknown-linux-gnu"
    ;;
  arm64)
    RUST_TARGET="aarch64-unknown-linux-gnu"
    ;;
  *)
    echo "❌ Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

# Build binary if not exists
if [[ ! -f "target/$RUST_TARGET/release/regent" ]]; then
  echo "Building regent for $RUST_TARGET..."
  cargo build --release --target "$RUST_TARGET"
fi

# Copy binary
cp "target/$RUST_TARGET/release/regent" "$BUILD_DIR/usr/bin/"
strip "$BUILD_DIR/usr/bin/regent"

# Create control file
cat > "$BUILD_DIR/DEBIAN/control" <<EOF
Package: regent
Version: $VERSION
Section: devel
Priority: optional
Architecture: $ARCH
Maintainer: Felipe Quintella <ffquintella@gmail.com>
Description: High-performance Puppet Development Kit
 Regent is a high-performance rebuild of PDK (Puppet Development Kit)
 in Rust, providing fast module building, testing, and validation.
 .
 Features:
  - Fast module building and packaging
  - Comprehensive testing framework
  - Validation and linting
  - Component generation
Homepage: https://github.com/seu-usuario/regent
EOF

# Create copyright file
mkdir -p "$BUILD_DIR/usr/share/doc/regent"
cat > "$BUILD_DIR/usr/share/doc/regent/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: regent
Source: https://github.com/seu-usuario/regent

Files: *
Copyright: 2025-2026 Felipe Quintella <ffquintella@gmail.com>
License: AGPL-3.0
 This program is free software: you can redistribute it and/or modify
 it under the terms of the GNU Affero General Public License as published by
 the Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.
 .
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU Affero General Public License for more details.
 .
 You should have received a copy of the GNU Affero General Public License
 along with this program.  If not, see <https://www.gnu.org/licenses/>.
EOF

# Create changelog
cat > "$BUILD_DIR/usr/share/doc/regent/changelog.Debian" <<EOF
regent ($VERSION) unstable; urgency=medium

  * Initial Debian package release
  * Full PDK functionality implemented
  * Cross-platform support

 -- Felipe Quintella <ffquintella@gmail.com>  $(date -R)
EOF
gzip -9n "$BUILD_DIR/usr/share/doc/regent/changelog.Debian"

# Build package
dpkg-deb --build --root-owner-group "$BUILD_DIR" "regent_${VERSION}_${ARCH}.deb"

echo "✅ Package created: regent_${VERSION}_${ARCH}.deb"
echo ""
echo "📝 Test installation with:"
echo "  sudo dpkg -i regent_${VERSION}_${ARCH}.deb"
echo "  regent --version"
echo ""
echo "📝 Uninstall with:"
echo "  sudo dpkg -r regent"
