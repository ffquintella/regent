# Regent build and semantic versioning
# Requirements: make, python3, cargo, npm

CARGO ?= cargo
PYTHON ?= python3
NPM ?= npm

.PHONY: help version-show bump-major bump-minor bump-patch build install vscode-extension

help:
	@echo "Available targets:"
	@echo "  version-show     - Show current version from Cargo.toml"
	@echo "  bump-major       - Increment MAJOR version (X+1.0.0)"
	@echo "  bump-minor       - Increment MINOR version (X.Y+1.0)"
	@echo "  bump-patch       - Increment PATCH version (X.Y.Z+1)"
	@echo "  build            - Build release binary (cargo build --release)"
	@echo "  install          - Install binary locally (cargo install --path . --locked)"
	@echo "  vscode-extension - Build VS Code extension VSIX package"

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
