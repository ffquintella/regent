"""Regression tests for the offline cache preparer, without Ruby or network."""

import importlib.util
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("prepare_gem_cache", Path(__file__).with_name("prepare-gem-cache.py"))
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


class PrepareCacheTests(unittest.TestCase):
    def extract(self, member):
        payload = io.BytesIO()
        with tarfile.open(fileobj=payload, mode="w") as archive:
            archive.addfile(member, io.BytesIO(b"a") if member.isfile() else None)
        payload.seek(0)
        with tempfile.TemporaryDirectory() as tmp, tarfile.open(fileobj=payload, mode="r:") as archive:
            cache.safe_unpack(archive, Path(tmp))
            if member.isfile():
                self.assertEqual((Path(tmp) / member.name).read_bytes(), b"a")

    def test_rejects_archive_traversal_and_windows_paths(self):
        for name in ("../escape", "/absolute", "nested/../../escape", "C:/escape", "nested\\escape"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                info = tarfile.TarInfo(name)
                info.size = 1
                self.extract(info)

    def test_rejects_links_and_special_files(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.CHRTYPE, tarfile.FIFOTYPE):
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                info = tarfile.TarInfo("unsafe")
                info.type = kind
                info.linkname = "../outside"
                self.extract(info)

    def test_safe_regular_file_extracts(self):
        info = tarfile.TarInfo("nested/lib/example.rb")
        info.size = 1
        info.mode = 0o6755
        self.extract(info)

    def test_archive_checksum_mismatch_fails_before_unpack(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "cache.tar.gz"
            archive.write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "invalid"):
                cache.verify_archive({"cache_sha256": "0" * 64}, archive)

    @unittest.skipIf(os.name == "nt", "POSIX executable mode regression")
    def test_staged_cache_is_data_without_executable_shebangs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "source"
            (source / "ruby").mkdir(parents=True)
            script = source / "ruby/helper"
            script.write_text("#!/sbin/sh\n")
            script.chmod(0o755)
            (source / "manifest.json").write_text("{}")
            target = root / "target"
            cache.replace_tree(source, target)
            self.assertEqual((target / "ruby/helper").stat().st_mode & 0o777, 0o644)
            self.assertEqual((target / "ruby").stat().st_mode & 0o777, 0o755)

    def test_version_bounds_and_runtime_dependency_closure(self):
        self.assertTrue(cache.satisfies("3.13.7", "~> 3.13.0"))
        self.assertFalse(cache.satisfies("3.14.0", "~> 3.13.0"))
        self.assertTrue(cache.satisfies("1.15.0", "~> 1.10"))
        self.assertTrue(cache.satisfies("2.5.7", "> 2.0.1, < 5"))
        self.assertFalse(cache.satisfies("5.0.0", "> 2.0.1, < 5"))
        lock = json.loads(cache.LOCK.read_text())
        cache.verify_lock(lock)
        lock["gems"] = [gem for gem in lock["gems"] if gem["name"] != "rspec-support"]
        with self.assertRaisesRegex(ValueError, "Unsatisfied runtime dependency"):
            cache.verify_lock(lock)

    def test_manifest_detects_payload_tampering(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "file.rb").write_text("original")
            lock = {"gems": [], "ruby_api": "2.6.0"}
            manifest = dict(lock, files={"file.rb": cache.digest(root / "file.rb")})
            (root / "manifest.json").write_text(json.dumps(manifest))
            cache.verify_tree(root, lock)
            (root / "file.rb").write_text("tampered")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                cache.verify_tree(root, lock)


if __name__ == "__main__":
    unittest.main()
