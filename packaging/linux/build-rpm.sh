#!/usr/bin/env bash
# Generate RPM package for Regent
# Requires: rpmbuild

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"
python3 scripts/prepare-gem-cache.py

VERSION="${1:-0.1.1}"
ARCH="${2:-x86_64}"  # x86_64 or aarch64
BUILD_DIR="$HOME/rpmbuild"

echo "📦 Building RPM package for regent v$VERSION ($ARCH)..."

# Create RPM build directory structure
mkdir -p "$BUILD_DIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Map Rust targets
case "$ARCH" in
  x86_64)
    RUST_TARGET="x86_64-unknown-linux-gnu"
    ;;
  aarch64)
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

# Stage both binary and cache for the source tarball
SOURCE_DIR="target/rpm-source-$ARCH/regent-$VERSION"
rm -rf "$SOURCE_DIR"
mkdir -p "$SOURCE_DIR"
cp "target/$RUST_TARGET/release/regent" "$SOURCE_DIR/"
python3 scripts/prepare-gem-cache.py --stage "$SOURCE_DIR/bundled_gems"

# Create source tarball
TARBALL="regent-$VERSION.tar.gz"
tar -czf "$BUILD_DIR/SOURCES/$TARBALL" \
  -C "$(dirname "$SOURCE_DIR")" "regent-$VERSION"

# Create spec file
cat > "$BUILD_DIR/SPECS/regent.spec" <<EOF
# The release binary is prebuilt; host strip may not support the target architecture.
%global __strip /bin/true
%global debug_package %{nil}
# Vendored Ruby is data for Artichoke, never a dependency on host interpreters.
%global __requires_exclude_from ^%{_datadir}/regent/bundled_gems/.*$

Name:           regent
Version:        $VERSION
Release:        1%{?dist}
Summary:        High-performance Puppet Development Kit

License:        AGPL-3.0
URL:            https://github.com/seu-usuario/regent
Source0:        %{name}-%{version}.tar.gz

BuildArch:      $ARCH
Requires:       glibc

%description
Regent is a high-performance rebuild of PDK (Puppet Development Kit)
in Rust, providing fast module building, testing, and validation.

Features:
- Fast module building and packaging
- Comprehensive testing framework
- Validation and linting
- Component generation

%prep
%setup -q

%build
# Binary already built

%install
mkdir -p %{buildroot}%{_bindir}
install -m 0755 regent %{buildroot}%{_bindir}/regent
mkdir -p %{buildroot}%{_datadir}/regent
cp -R bundled_gems %{buildroot}%{_datadir}/regent/

%files
%{_bindir}/regent
%{_datadir}/regent

%changelog
* $(date "+%a %b %d %Y") Felipe Quintella <ffquintella@gmail.com> - $VERSION-1
- Initial RPM release
- Full PDK functionality implemented
- Cross-platform support
EOF

# Build RPM
rpmbuild -ba --target "$ARCH" "$BUILD_DIR/SPECS/regent.spec"

# Copy to current directory
cp "$BUILD_DIR/RPMS/$ARCH/regent-$VERSION-1."*".rpm" .

echo "✅ Package created: regent-$VERSION-1.$ARCH.rpm"
echo ""
echo "📝 Test installation with:"
echo "  sudo rpm -ivh regent-$VERSION-1.$ARCH.rpm"
echo "  regent --version"
echo ""
echo "📝 Uninstall with:"
echo "  sudo rpm -e regent"
