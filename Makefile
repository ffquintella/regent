# Regent build and semantic versioning
# Requirements: make, python3, cargo, npm

CARGO ?= cargo
PYTHON ?= python3
NPM ?= npm

.PHONY: help version-show bump-major bump-minor bump-patch build install vscode-extension \
	package-homebrew package-deb package-rpm package-windows package-all

help:
	@echo "Available targets:"
	@echo "  version-show     - Show current version from Cargo.toml"
	@echo "  bump-major       - Increment MAJOR version (X+1.0.0)"
	@echo "  bump-minor       - Increment MINOR version (X.Y+1.0)"
	@echo "  bump-patch       - Increment PATCH version (X.Y.Z+1)"
	@echo "  build            - Build release binary (cargo build --release)"
	@echo "  install          - Install binary locally (cargo install --path . --locked)"
	@echo "  vscode-extension - Build VS Code extension VSIX package"
	@echo ""
	@echo "Packaging targets:"
	@echo "  package-homebrew - Build Homebrew release artifacts"
	@echo "  package-deb      - Build Debian packages (amd64, arm64)"
	@echo "  package-rpm      - Build RPM packages (x86_64, aarch64)"
	@echo "  package-windows  - Build Windows MSI and portable ZIP (requires Windows)"
	@echo "  package-all      - Build all packages (runs on current platform)"

version-show:
	@$(PYTHON) -c 'import pathlib, re, sys; text = pathlib.Path("Cargo.toml").read_text(); m = re.search(r"^version = \"([0-9]+\.[0-9]+\.[0-9]+)\"", text, re.M); print(m.group(1)) if m else sys.exit("version not found in Cargo.toml")'

bump-major:
	@$(PYTHON) -c 'import pathlib, re, sys; path = pathlib.Path("Cargo.toml"); text = path.read_text(); m = re.search(r"^version = \"([0-9]+)\.([0-9]+)\.([0-9]+)\"", text, re.M); sys.exit("version not found in Cargo.toml") if not m else None; maj, minor, patch = map(int, m.groups()); new_version = f"{maj + 1}.0.0"; old_version = f"{maj}.{minor}.{patch}"; updated = text.replace(f"version = \"{old_version}\"", f"version = \"{new_version}\"", 1); path.write_text(updated); print(f"Bumped MAJOR: {old_version} -> {new_version}")'

bump-minor:
	@$(PYTHON) -c 'import pathlib, re, sys; path = pathlib.Path("Cargo.toml"); text = path.read_text(); m = re.search(r"^version = \"([0-9]+)\.([0-9]+)\.([0-9]+)\"", text, re.M); sys.exit("version not found in Cargo.toml") if not m else None; maj, minor, patch = map(int, m.groups()); new_version = f"{maj}.{minor + 1}.0"; old_version = f"{maj}.{minor}.{patch}"; updated = text.replace(f"version = \"{old_version}\"", f"version = \"{new_version}\"", 1); path.write_text(updated); print(f"Bumped MINOR: {old_version} -> {new_version}")'

bump-patch:
	@$(PYTHON) -c 'import pathlib, re, sys; path = pathlib.Path("Cargo.toml"); text = path.read_text(); m = re.search(r"^version = \"([0-9]+)\.([0-9]+)\.([0-9]+)\"", text, re.M); sys.exit("version not found in Cargo.toml") if not m else None; maj, minor, patch = map(int, m.groups()); new_version = f"{maj}.{minor}.{patch + 1}"; old_version = f"{maj}.{minor}.{patch}"; updated = text.replace(f"version = \"{old_version}\"", f"version = \"{new_version}\"", 1); path.write_text(updated); print(f"Bumped PATCH: {old_version} -> {new_version}")'

build:
	$(CARGO) build --release


vscode-extension:
	@echo "Building VS Code extension..."
	cd vscode-extension && $(NPM) install
	cd vscode-extension && $(NPM) run compile
	cd vscode-extension && $(NPM) run package
	@echo "VSIX package created in vscode-extension/"
	@ls -lh vscode-extension/*.vsix
install:
	$(CARGO) install --path . --locked

# Packaging targets
package-homebrew:
	@echo "Building Homebrew release artifacts..."
	@bash packaging/homebrew/build-release.sh $$($(MAKE) version-show)
	@echo "Homebrew artifacts ready in target/homebrew-release/"

package-deb:
	@echo "Building Debian packages..."
	@bash packaging/linux/build-deb.sh $$($(MAKE) version-show) amd64
	@bash packaging/linux/build-deb.sh $$($(MAKE) version-show) arm64
	@echo "Debian packages ready"

package-rpm:
	@echo "Building RPM packages..."
	@bash packaging/linux/build-rpm.sh $$($(MAKE) version-show) x86_64
	@bash packaging/linux/build-rpm.sh $$($(MAKE) version-show) aarch64
	@echo "RPM packages ready"

package-windows:
	@echo "Building Windows packages..."
	@echo "Note: This target should be run on Windows"
	@echo "Run: cd packaging\\windows && build-msi.bat && build-portable.bat"

package-all: package-homebrew package-deb package-rpm
	@echo "All packages built successfully!"
	@echo "Windows packages must be built separately on Windows platform"
