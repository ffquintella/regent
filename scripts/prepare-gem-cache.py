#!/usr/bin/env python3
"""Verify/extract Regent's prebuilt cache; --rebuild downloads pinned archives.

Normal preparation and staging are offline and use only Python's standard
library. Ruby metadata is retained as data, never evaluated.
"""

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import sys
import tarfile
import tempfile
import urllib.request

REPO = Path(__file__).resolve().parents[1]
CACHE = REPO / "assets" / "bundled_gems"
LOCK = CACHE / "cache.lock.json"
ARCHIVE = CACHE / "cache.tar.gz"


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def safe_unpack(archive, destination):
    """No links, device files, absolute paths or traversal in upstream data."""
    for member in archive:
        path = PurePosixPath(member.name)
        if (path.is_absolute() or ".." in path.parts or "\\" in member.name
                or ":" in member.name or not (member.isfile() or member.isdir())):
            raise ValueError(f"Unsafe archive member: {member.name}")
        target = destination.joinpath(*path.parts)
        if member.isdir():
            target.mkdir(parents=True, exist_ok=True)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
            # Upstream executable bits are useful; discard special permission bits.
            target.chmod(0o755 if member.mode & 0o111 else 0o644)


def verify_tree(root, lock):
    manifest = json.loads((root / "manifest.json").read_text())
    if manifest["gems"] != lock["gems"] or manifest["ruby_api"] != lock["ruby_api"]:
        raise ValueError("Cache manifest does not match the pinned lock")
    actual = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file()}
    if actual != set(manifest["files"]) | {"manifest.json"}:
        raise ValueError("Cache has missing or unexpected files")
    for name, expected in manifest["files"].items():
        path = PurePosixPath(name)
        if path.is_absolute() or ".." in path.parts or "\\" in name or ":" in name:
            raise ValueError(f"Unsafe manifest path: {name}")
        source = root.joinpath(*path.parts)
        if source.is_symlink() or digest(source) != expected:
            raise ValueError(f"Cache file checksum mismatch: {name}")
    for gem in lock["gems"]:
        entry = root / "ruby" / lock["ruby_api"] / "gems" / f"{gem['name']}-{gem['version']}" / gem["entrypoint"]
        if not entry.is_file():
            raise ValueError(f"Missing gem entrypoint: {entry}")


def verify_archive(lock, archive=ARCHIVE):
    if not archive.is_file() or digest(archive) != lock.get("cache_sha256"):
        raise ValueError(f"Missing or invalid {archive}; restore the committed prebuilt cache")


def unpack_prebuilt(lock, destination):
    verify_archive(lock)
    with tarfile.open(ARCHIVE, "r:gz") as archive:
        safe_unpack(archive, destination)
    verify_tree(destination, lock)


def replace_tree(source, destination):
    destination.mkdir(parents=True, exist_ok=True)
    # Only replace our cache payload; leave README, lock, and archive intact.
    for name in ("ruby", "manifest.json"):
        target = destination / name
        if target.is_symlink():
            raise ValueError(f"Refusing to replace symlink: {target}")
        if target.is_dir():
            shutil.rmtree(target)
        elif target.exists():
            target.unlink()
        if (source / name).is_dir():
            shutil.copytree(source / name, target)
        else:
            shutil.copy2(source / name, target)
        # Cache code is Artichoke input data, not a set of host executables.
        # In particular, RPM must not infer Ruby/Solaris interpreter deps from
        # upstream helper scripts' shebangs or rewrite those scripts.
        if target.is_dir():
            for path in target.rglob("*"):
                path.chmod(0o755 if path.is_dir() else 0o644)
            target.chmod(0o755)
        else:
            target.chmod(0o644)


def satisfies(version, requirements):
    """The lock uses release versions and RubyGems numeric comparisons only."""
    def parts(value):
        if not re.fullmatch(r"\d+(?:\.\d+)*", value):
            raise ValueError(f"Unsupported version in lock: {value}")
        values = tuple(map(int, value.split(".")))
        return values + (0,) * (4 - len(values))
    actual = parts(version)
    for requirement in requirements.split(","):
        match = re.fullmatch(r"\s*(~>|>=|<=|>|<|=)?\s*(\d+(?:\.\d+)*)\s*", requirement)
        if not match:
            raise ValueError(f"Unsupported requirement: {requirement}")
        operator, value = match.groups()
        wanted = parts(value)
        comparisons = {"=": actual == wanted, ">=": actual >= wanted,
                       "<=": actual <= wanted, ">": actual > wanted, "<": actual < wanted}
        if operator == "~>":
            upper = list(map(int, value.split(".")))
            index = max(0, len(upper) - 2)
            upper[index] += 1
            upper = upper[:index + 1]
            valid = wanted <= actual < parts(".".join(map(str, upper)))
        else:
            valid = comparisons[operator or "="]
        if not valid:
            return False
    return True


def verify_lock(lock):
    gems = {gem["name"]: gem for gem in lock["gems"]}
    if len(gems) != len(lock["gems"]):
        raise ValueError("Duplicate gem names in lock")
    for name, gem in gems.items():
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name) or not re.fullmatch(r"\d+(?:\.\d+)*", gem["version"]):
            raise ValueError("Invalid locked gem name/version")
        if not re.fullmatch(r"[0-9a-f]{64}", gem["sha256"]):
            raise ValueError(f"Invalid archive checksum for {name}")
        for dep in gem["dependencies"]:
            candidate = gems.get(dep["name"])
            if not candidate or not satisfies(candidate["version"], dep["requirements"]):
                raise ValueError(f"Unsatisfied runtime dependency: {name} -> {dep}")


def rebuild(lock):
    """Maintainer operation only. Fetch signed-off pins, never resolve latest."""
    with tempfile.TemporaryDirectory(prefix="regent-gems-") as staging:
        root = Path(staging)
        for gem in lock["gems"]:
            fullname = f"{gem['name']}-{gem['version']}"
            url = f"https://rubygems.org/downloads/{fullname}.gem"
            print(f"Fetching {fullname}", flush=True)
            with urllib.request.urlopen(url, timeout=60) as response:
                data = response.read()
            if hashlib.sha256(data).hexdigest() != gem["sha256"]:
                raise ValueError(f"Upstream archive checksum mismatch: {fullname}")
            with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as container:
                metadata = gzip.decompress(container.extractfile("metadata.gz").read())
                # Ruby object tags remain opaque. Reject native-extension build hooks.
                if not re.search(rb"(?m)^extensions: \[\]\s*$", metadata):
                    raise ValueError(f"Gem declares native extensions: {fullname}")
                payload = container.extractfile("data.tar.gz").read()
            gemdir = root / "ruby" / lock["ruby_api"] / "gems" / fullname
            gemdir.mkdir(parents=True)
            with tarfile.open(fileobj=io.BytesIO(payload), mode="r:gz") as archive:
                safe_unpack(archive, gemdir)
            specs = root / "ruby" / lock["ruby_api"] / "specifications"
            specs.mkdir(parents=True, exist_ok=True)
            (specs / f"{fullname}.yaml").write_bytes(metadata)
            paths_match = re.search(rb"(?m)^require_paths:\n((?:- [^\n]+\n)+)", metadata)
            if not paths_match:
                raise ValueError(f"Missing require paths in metadata: {fullname}")
            require_paths = [line[2:].decode("utf-8") for line in paths_match[1].splitlines()]
            # Keep conventional specs for tools that inspect the cache; no Ruby execution.
            lines = ["Gem::Specification.new do |s|", f"  s.name = {json.dumps(gem['name'])}",
                     f"  s.version = {json.dumps(gem['version'])}",
                     "  s.summary = 'Regent pinned offline dependency'", "  s.authors = ['Upstream authors; see metadata YAML']",
                     f"  s.require_paths = {json.dumps(require_paths)}", f"  s.licenses = {json.dumps(gem['licenses'])}"]
            for dep in gem["dependencies"]:
                lines.append(f"  s.add_runtime_dependency({json.dumps(dep['name'])}, {', '.join(json.dumps(r.strip()) for r in dep['requirements'].split(','))})")
            lines.append("end\n")
            (specs / f"{fullname}.gemspec").write_text("\n".join(lines), encoding="utf-8")
        files = {p.relative_to(root).as_posix(): digest(p) for p in sorted(root.rglob("*")) if p.is_file()}
        manifest = {"ruby_api": lock["ruby_api"], "gems": lock["gems"], "files": files}
        (root / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        verify_tree(root, lock)
        # Deterministic metadata and gzip timestamp make rebuilds reproducible.
        temporary = ARCHIVE.with_suffix(".tmp")
        with temporary.open("wb") as output, gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                for path in sorted(root.rglob("*")):
                    info = archive.gettarinfo(str(path), path.relative_to(root).as_posix())
                    info.uid = info.gid = info.mtime = 0
                    info.uname = info.gname = ""
                    if path.is_file():
                        with path.open("rb") as source:
                            archive.addfile(info, source)
                    else:
                        info.mode = 0o755
                        archive.addfile(info)
        os.replace(temporary, ARCHIVE)
        lock["cache_sha256"] = digest(ARCHIVE)
        LOCK.write_text(json.dumps(lock, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the committed archive without modifying files")
    parser.add_argument("--stage", type=Path, help="copy a verified payload to a packaging directory")
    parser.add_argument("--rebuild", action="store_true", help="maintainer: download locked .gem archives and rebuild the prebuilt cache")
    args = parser.parse_args()
    if args.check and (args.stage or args.rebuild):
        parser.error("--check cannot be combined with --stage or --rebuild")
    lock = json.loads(LOCK.read_text())
    verify_lock(lock)
    if args.rebuild:
        rebuild(lock)
    with tempfile.TemporaryDirectory(prefix="regent-cache-") as staging:
        root = Path(staging)
        unpack_prebuilt(lock, root)
        if not args.check:
            replace_tree(root, args.stage.resolve() if args.stage else CACHE)
    print(f"Verified Regent cache: {len(lock['gems'])} pinned gems")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        print(f"Gem cache error: {error}", file=sys.stderr)
        sys.exit(1)
