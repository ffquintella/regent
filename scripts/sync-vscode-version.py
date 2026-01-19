#!/usr/bin/env python3
import argparse
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CARGO_TOML = ROOT / "Cargo.toml"
PACKAGE_JSON = ROOT / "vscode-extension" / "package.json"
PACKAGE_LOCK = ROOT / "vscode-extension" / "package-lock.json"


def read_cargo_version() -> str:
    text = CARGO_TOML.read_text()
    match = re.search(r'^version\s*=\s*"([0-9A-Za-z.\-+]+)"', text, re.M)
    if not match:
        raise RuntimeError("version not found in Cargo.toml")
    return match.group(1)


def replace_once(pattern: str, repl: str, text: str, *, flags: int = 0) -> str:
    updated, count = re.subn(pattern, repl, text, count=1, flags=flags)
    if count != 1:
        raise RuntimeError(f"pattern not found: {pattern}")
    return updated


def check_versions(expected: str) -> None:
    pkg = json.loads(PACKAGE_JSON.read_text())
    lock = json.loads(PACKAGE_LOCK.read_text())
    pkg_version = pkg.get("version")
    lock_version = lock.get("version")
    lock_pkg_version = lock.get("packages", {}).get("", {}).get("version")

    mismatches = []
    if pkg_version != expected:
        mismatches.append(f"package.json version is {pkg_version}")
    if lock_version != expected:
        mismatches.append(f"package-lock.json version is {lock_version}")
    if lock_pkg_version != expected:
        mismatches.append(f"package-lock.json packages[''] version is {lock_pkg_version}")

    if mismatches:
        details = "; ".join(mismatches)
        raise RuntimeError(f"VS Code extension versions do not match Cargo.toml ({expected}): {details}")


def sync_versions(expected: str) -> None:
    pkg_text = PACKAGE_JSON.read_text()
    pkg_text = replace_once(
        r'("version"\s*:\s*")([^"]+)(")',
        r'\g<1>' + expected + r'\g<3>',
        pkg_text,
    )
    if pkg_text != PACKAGE_JSON.read_text():
        PACKAGE_JSON.write_text(pkg_text)

    lock_text = PACKAGE_LOCK.read_text()
    lock_text = replace_once(
        r'("version"\s*:\s*")([^"]+)(")',
        r'\g<1>' + expected + r'\g<3>',
        lock_text,
    )
    lock_text = replace_once(
        r'("packages"\s*:\s*{\s*""\s*:\s*{.*?"version"\s*:\s*")([^"]+)(")',
        r'\g<1>' + expected + r'\g<3>',
        lock_text,
        flags=re.S,
    )
    if lock_text != PACKAGE_LOCK.read_text():
        PACKAGE_LOCK.write_text(lock_text)


def main() -> int:
    parser = argparse.ArgumentParser(description="Sync VS Code extension version with Cargo.toml.")
    parser.add_argument("--check", action="store_true", help="Fail if versions are out of sync.")
    args = parser.parse_args()

    expected = read_cargo_version()

    if args.check:
        check_versions(expected)
        return 0

    sync_versions(expected)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
