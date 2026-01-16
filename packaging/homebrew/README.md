# Regent Homebrew Packaging

This directory contains scripts and formula for distributing Regent via Homebrew.

## Quick Start

### For Users

```bash
# Install from local formula (development)
brew install --build-from-source ./packaging/homebrew/regent.rb

# Or from a tap (once published)
brew tap seu-usuario/regent
brew install regent
```

### For Maintainers

#### 1. Build Release Artifacts

```bash
./packaging/homebrew/build-release.sh 0.1.1
```

This creates:
- Release binaries for macOS (x86_64, ARM64) and Linux (x86_64, ARM64)
- Compressed tarballs
- SHA256 checksums

#### 2. Upload to GitHub Releases

```bash
# Upload all tarballs to GitHub releases
gh release create v0.1.1 \
  target/homebrew-release/regent-0.1.1-*.tar.gz \
  --title "v0.1.1" \
  --notes "Release notes here"
```

#### 3. Update Formula

```bash
./packaging/homebrew/update-formula.sh 0.1.1
```

Then manually edit `regent.rb` to insert the correct SHA256 checksums.

#### 4. Test Installation

```bash
# Test local formula
brew install --build-from-source ./packaging/homebrew/regent.rb

# Verify
regent --version
regent --help

# Uninstall
brew uninstall regent
```

## Formula Structure

The `regent.rb` formula:

- **Multi-platform support**: macOS (Intel/ARM) and Linux (x86_64/ARM64)
- **Binary installation**: Direct binary install (no compilation needed)
- **Shell completions**: Bash, Zsh, Fish support
- **Man pages**: Automatic installation if available
- **Verification**: Basic smoke tests

## Publishing to Homebrew

### Option 1: Personal Tap (Recommended for Initial Release)

```bash
# Create tap repository
gh repo create homebrew-regent --public

# Clone and add formula
git clone https://github.com/seu-usuario/homebrew-regent.git
cp packaging/homebrew/regent.rb homebrew-regent/Formula/
cd homebrew-regent
git add Formula/regent.rb
git commit -m "Add regent formula"
git push

# Users can now install with:
# brew tap seu-usuario/regent
# brew install regent
```

### Option 2: Submit to Homebrew Core (For Stable Releases)

1. Ensure project meets [acceptable formulae guidelines](https://docs.brew.sh/Acceptable-Formulae)
2. Fork [homebrew-core](https://github.com/Homebrew/homebrew-core)
3. Add formula to `Formula/regent.rb`
4. Submit pull request
5. Address reviewer feedback

## Release Checklist

- [ ] Update version in `Cargo.toml`
- [ ] Update version in `regent.rb`
- [ ] Run `build-release.sh` for all targets
- [ ] Create GitHub release with artifacts
- [ ] Update SHA256 checksums in formula
- [ ] Test installation locally
- [ ] Test on clean system (Docker/VM)
- [ ] Update tap repository
- [ ] Announce release

## Troubleshooting

### Build fails for specific target

```bash
# Install cross-compilation tools
rustup target add aarch64-apple-darwin
rustup target add x86_64-unknown-linux-gnu

# For Linux builds on macOS, consider using cross:
cargo install cross
cross build --release --target x86_64-unknown-linux-gnu
```

### SHA256 mismatch

```bash
# Regenerate checksums
sha256sum target/homebrew-release/regent-0.1.1-*.tar.gz
```

### Formula validation errors

```bash
# Audit formula
brew audit --strict --online ./packaging/homebrew/regent.rb

# Test installation
brew install --build-from-source --verbose --debug ./packaging/homebrew/regent.rb
```

## Files

- `regent.rb` - Homebrew formula
- `build-release.sh` - Build script for all platforms
- `update-formula.sh` - Helper to update SHA256 checksums
- `README.md` - This file

## Resources

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew Acceptable Formulae](https://docs.brew.sh/Acceptable-Formulae)
- [Creating Taps](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)
