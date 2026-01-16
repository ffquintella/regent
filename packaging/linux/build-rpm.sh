#!/usr/bin/env bash
# Generate RPM package for Regent
# Requires: rpmbuild

set -euo pipefail

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

# Create source tarball
TARBALL="regent-$VERSION.tar.gz"
tar -czf "$BUILD_DIR/SOURCES/$TARBALL" \
  --transform="s,^,regent-$VERSION/," \
  -C target/$RUST_TARGET/release regent

# Create spec file
cat > "$BUILD_DIR/SPECS/regent.spec" <<EOF
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

%files
%{_bindir}/regent

%changelog
* $(date "+%a %b %d %Y") Felipe Quintella <ffquintella@gmail.com> - $VERSION-1
- Initial RPM release
- Full PDK functionality implemented
- Cross-platform support
EOF

# Build RPM
rpmbuild -ba "$BUILD_DIR/SPECS/regent.spec"

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
