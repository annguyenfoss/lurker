#!/usr/bin/env python3
"""Input consistency controls only: no compiler, LKL/FUSE/storage or install."""
from pathlib import Path
import importlib.util
import json
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "macos_lkl_builder", Path(__file__).with_name("build-macos-lkl-filesystem-driver.py"))
builder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(builder)


class ReleasedInputs(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="macos-lkl-input-control-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.include = self.root / "include"
        self.include.mkdir()
        for name in ("lkl.h", "lkl_host.h"):
            (self.include / name).write_text("controlled exported header " + name)
        self.profile = self.source / "tools/lkl/darwin/profile"
        self.profile.mkdir(parents=True)
        (self.profile / "kernel.config").write_text("controlled profile; not a kernel build")
        for name in builder.RELEASE_BASES:
            p = self.source / name
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("controlled host input " + name)
        self.library = self.root / "library.a"
        self.library.write_bytes(b"controlled archive marker, never linked")
        self.provenance = self.root / "provenance.json"
        self.value = {
            "lkl_commit": builder.LKL_COMMIT,
            "verified_git_tree": builder.LKL_TREE,
            "unchanged_profile": True,
            "library_sha256": builder.sha256(self.library),
            "headers": {p.name: builder.sha256(p) for p in self.include.iterdir()},
            "profile": {"kernel.config": builder.sha256(self.profile / "kernel.config")},
            "pre_patch_sha256": builder.RELEASE_BASES,
            "post_patch_sha256": {n: builder.sha256(self.source / n)
                                  for n in builder.RELEASE_BASES},
        }

    def verify(self):
        self.provenance.write_text(json.dumps(self.value))
        return builder.release_provenance(self.source, self.library, self.include,
                                          self.provenance, builder.sha256(self.provenance))

    def test_exact_archive_source_inputs_without_git(self):
        self.assertFalse((self.source / ".git").exists())
        self.assertEqual(self.verify(), self.value)

    def test_wrong_explicit_provenance_hash(self):
        self.provenance.write_text(json.dumps(self.value))
        with self.assertRaises(ValueError):
            builder.release_provenance(self.source, self.library, self.include,
                                       self.provenance, "0" * 64)

    def test_historical_pin_cannot_be_relabeled(self):
        self.value["lkl_commit"] = "c4d0895ead9badb52f7bbffe3d529c61c8fc9485"
        with self.assertRaises(ValueError): self.verify()

    def test_wrong_release_tree(self):
        self.value["verified_git_tree"] = "0" * 40
        with self.assertRaises(ValueError): self.verify()

    def test_changed_library(self):
        self.library.write_bytes(b"different archive marker")
        with self.assertRaises(ValueError): self.verify()

    def test_extra_exported_header(self):
        (self.include / "extra.h").write_text("unreviewed")
        with self.assertRaises(ValueError): self.verify()

    def test_changed_exported_header(self):
        (self.include / "lkl.h").write_text("changed")
        with self.assertRaises(ValueError): self.verify()

    def test_changed_released_profile(self):
        (self.profile / "kernel.config").write_text("changed")
        with self.assertRaises(ValueError): self.verify()

    def test_changed_host_link_input(self):
        (self.source / "tools/lkl/darwin/build.py").write_text("changed")
        with self.assertRaises(ValueError): self.verify()

    def test_no_unreviewed_source_paths(self):
        self.value["post_patch_sha256"]["../../outside"] = "0" * 64
        with self.assertRaises(ValueError): self.verify()

    def test_profile_change_is_not_unchanged(self):
        self.value["unchanged_profile"] = False
        with self.assertRaises(ValueError): self.verify()


if __name__ == "__main__":
    unittest.main()
