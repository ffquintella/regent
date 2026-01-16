# Cross-Platform Packaging for Regent

This directory contains packaging configurations for distributing Regent across multiple platforms.

## Supported Platforms

| Platform | Package Type | Status |
|----------|--------------|--------|
| **macOS** | Homebrew | ✅ Ready |
| **Linux (Debian/Ubuntu)** | .deb | ✅ Ready |
| **Linux (Fedora/RHEL)** | .rpm | ✅ Ready |
| **Windows** | MSI | ✅ Ready |
| **Windows** | Portable ZIP | ✅ Ready |

## Quick Start

### macOS

```bash
# Build Homebrew release
./packaging/homebrew/build-release.sh 0.1.1

# Test locally
brew install --build-from-source ./packaging/homebrew/regent.rb
```

### Linux - Debian/Ubuntu

```bash
# Build package
./packaging/linux/build-deb.sh 0.1.1 amd64

# Install
sudo dpkg -i regent_0.1.1_amd64.deb
```

### Linux - Fedora/RHEL

```bash
# Build package
./packaging/linux/build-rpm.sh 0.1.1 x86_64

# Install
sudo rpm -ivh regent-0.1.1-1.x86_64.rpm
```

### Windows

```batch
REM Build MSI installer
cd packaging\windows
build-msi.bat

REM Or build portable ZIP
build-portable.bat
```

## Directory Structure

```
packaging/
├── homebrew/           # macOS Homebrew formula
│   ├── regent.rb      # Formula definition
│   ├── build-release.sh
│   ├── update-formula.sh
│   └── README.md
├── linux/             # Linux packages
│   ├── build-deb.sh  # Debian package builder
│   ├── build-rpm.sh  # RPM package builder
│   └── README.md
├── windows/           # Windows installers
│   ├── regent.wxs    # WiX installer definition
│   ├── build-msi.bat # MSI builder
│   ├── build-portable.bat
│   └── README.md
└── README.md         # This file
```

## Release Process

### 1. Version Update

Update version in:
- `Cargo.toml`
- `lib/regent/version.rb`
- `packaging/homebrew/regent.rb`
- `packaging/windows/regent.wxs`
- `packaging/windows/build-*.bat`

### 2. Build All Packages

```bash
# Set version
VERSION=0.1.1

# macOS
./packaging/homebrew/build-release.sh $VERSION

# Linux - Debian
./packaging/linux/build-deb.sh $VERSION amd64
./packaging/linux/build-deb.sh $VERSION arm64

# Linux - RPM
./packaging/linux/build-rpm.sh $VERSION x86_64
./packaging/linux/build-rpm.sh $VERSION aarch64
```

On Windows machine:
```batch
cd packaging\windows
build-msi.bat
build-portable.bat
```

### 3. Test Packages

Test on each platform:

```bash
# macOS
brew install --build-from-source ./packaging/homebrew/regent.rb
regent --version

# Debian/Ubuntu
docker run -it --rm -v $(pwd):/work debian:12 bash
cd /work && dpkg -i regent_0.1.1_amd64.deb
regent --version

# Fedora/RHEL
docker run -it --rm -v $(pwd):/work fedora:39 bash
cd /work && dnf install -y ./regent-0.1.1-1.x86_64.rpm
regent --version
```

### 4. Generate Checksums

```bash
# SHA256
sha256sum regent_0.1.1_amd64.deb > regent_0.1.1_amd64.deb.sha256
sha256sum regent-0.1.1-1.x86_64.rpm > regent-0.1.1-1.x86_64.rpm.sha256
sha256sum regent-0.1.1-windows-x64-portable.zip > regent-0.1.1-windows-x64-portable.zip.sha256

# Or all at once
find . -name "regent*" -type f \( -name "*.deb" -o -name "*.rpm" -o -name "*.msi" -o -name "*.zip" -o -name "*.tar.gz" \) -exec sha256sum {} \; > SHA256SUMS
```

### 5. Sign Packages (Production)

```bash
# Debian/RPM with GPG
gpg --detach-sign --armor regent_0.1.1_amd64.deb
gpg --detach-sign --armor regent-0.1.1-1.x86_64.rpm

# Windows with SignTool
signtool sign /f cert.pfx /p password regent-0.1.1-x64.msi

# macOS with codesign (for binaries)
codesign --sign "Developer ID Application" regent
```

### 6. Create GitHub Release

```bash
# Using GitHub CLI
gh release create v$VERSION \
  --title "Regent v$VERSION" \
  --notes-file CHANGELOG.md \
  packaging/homebrew/target/homebrew-release/*.tar.gz* \
  regent_0.1.1_*.deb* \
  regent-0.1.1-1.*.rpm* \
  regent-0.1.1-*.msi \
  regent-0.1.1-*-portable.zip \
  SHA256SUMS
```

### 7. Update Distribution Channels

#### Homebrew
```bash
# Update formula with new SHA256s
./packaging/homebrew/update-formula.sh $VERSION

# Test
brew install --build-from-source ./packaging/homebrew/regent.rb

# Submit to tap or homebrew-core
git clone https://github.com/seu-usuario/homebrew-regent
cp packaging/homebrew/regent.rb homebrew-regent/Formula/
cd homebrew-regent
git commit -am "Update regent to v$VERSION"
git push
```

#### Chocolatey (Windows)
```batch
choco pack regent.nuspec
choco push regent.0.1.1.nupkg
```

#### Cargo (Rust)
```bash
cargo publish
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Release
on:
  push:
    tags:
      - 'v*'

jobs:
  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - run: ./packaging/homebrew/build-release.sh ${GITHUB_REF#refs/tags/v}
      
  build-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: ./packaging/linux/build-deb.sh ${GITHUB_REF#refs/tags/v} amd64
      - run: ./packaging/linux/build-rpm.sh ${GITHUB_REF#refs/tags/v} x86_64
      
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - run: packaging\windows\build-msi.bat
      - run: packaging\windows\build-portable.bat
```

## Installation Instructions for Users

### macOS

```bash
# Via Homebrew tap
brew tap seu-usuario/regent
brew install regent

# Or from formula file
brew install --build-from-source https://raw.githubusercontent.com/seu-usuario/regent/main/packaging/homebrew/regent.rb
```

### Ubuntu/Debian

```bash
# Download and install
wget https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent_0.1.1_amd64.deb
sudo dpkg -i regent_0.1.1_amd64.deb

# Or from repository
echo "deb [trusted=yes] https://your-repo.com/apt stable main" | sudo tee /etc/apt/sources.list.d/regent.list
sudo apt update
sudo apt install regent
```

### Fedora/RHEL/CentOS

```bash
# Download and install
wget https://github.com/seu-usuario/regent/releases/download/v0.1.1/regent-0.1.1-1.x86_64.rpm
sudo dnf install ./regent-0.1.1-1.x86_64.rpm

# Or from repository
sudo tee /etc/yum.repos.d/regent.repo <<EOF
[regent]
name=Regent Repository
baseurl=https://your-repo.com/yum
enabled=1
gpgcheck=0
EOF
sudo dnf install regent
```

### Windows

**Option 1: MSI Installer**
1. Download `regent-0.1.1-x64.msi`
2. Double-click to install
3. Installer adds Regent to PATH automatically

**Option 2: Portable ZIP**
1. Download `regent-0.1.1-windows-x64-portable.zip`
2. Extract to desired location
3. Add folder to PATH manually

**Option 3: Chocolatey**
```powershell
choco install regent
```

**Option 4: Scoop**
```powershell
scoop bucket add regent https://github.com/seu-usuario/scoop-regent
scoop install regent
```

## Troubleshooting

See individual platform READMEs:
- [Homebrew](homebrew/README.md)
- [Linux](linux/README.md)
- [Windows](windows/README.md)

## Contributing

When adding new platforms or package types:

1. Create subdirectory under `packaging/`
2. Add build scripts
3. Add comprehensive README
4. Update this main README
5. Add CI/CD workflow
6. Test on target platform

## Resources

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Debian Packaging Guide](https://www.debian.org/doc/manuals/maint-guide/)
- [RPM Packaging Guide](https://rpm-packaging-guide.github.io/)
- [WiX Toolset Documentation](https://wixtoolset.org/documentation/)
