import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("linux_wx", Path(__file__).parents[1] / "linux_wx.py")
wx = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(wx)


class LinuxWxTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lurker-wx-unit-")
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"; self.source.mkdir()
        (self.source / "code.cpp").write_text("int stable_source;\n")
        (self.source / "configure").write_text("#!/bin/sh\nexit 0\n"); (self.source / "configure").chmod(0o755)
        self.archive = self.root / "source.tar.bz2"
        with tarfile.open(self.archive, "w:bz2") as stream: stream.add(self.source, arcname="wx-source")
        self.pin = mock.patch.object(wx, "SHA256", wx.digest(self.archive)); self.pin.start()
        self.prefix = self.root / "install-minimal"; (self.prefix / "lib/wx/config").mkdir(parents=True)
        (self.prefix / "lib/libexample.a").write_bytes(b"static archive bytes")
        (self.prefix / "header.h").write_text("#define BOUND 1\n")
        self.inputs = {"source": "immutable unit input", "compiler": "recorded compiler", "flags": ["minimal"]}

    def tearDown(self):
        self.pin.stop(); self.temporary.cleanup()

    def receipt(self):
        return {"schema": 1, "inputs": self.inputs, "files": wx.inventory(self.prefix)}

    def legacy_evidence(self):
        config = self.prefix / "lib/wx/config/base-unicode-static-3.2"; config.write_text("not executed")
        (self.prefix / "bin").mkdir(); (self.prefix / "bin/wx-config").symlink_to(config)
        directory = self.root / "minimal-build"; directory.mkdir()
        executable = str(Path(sys.executable).resolve())
        (directory / "Makefile").write_text("CC = " + executable + "\nCXX = " + executable + " -std=c++11\n")
        self.inputs = {"source_dir": str(self.source), "flags": ["--prefix=" + str(self.prefix), "--minimal"],
                       "environment": {name: "" for name in wx.ENV_FLAGS},
                       "compilers": {name: {"path": executable, "version": "unit compiler"} for name in ("CC", "CXX")}}
        old = {"source": [wx.URL, wx.SHA256], "source_dir": str(self.source), "flags": self.inputs["flags"], "compiler": "unit compiler"}
        artifacts = {**old, "files": {name: item["sha256"] for name, item in wx.inventory(self.prefix).items() if "sha256" in item}}
        (self.root / "minimal-build-spec.json").write_text(json.dumps(old))
        (self.root / "minimal-artifacts.json").write_text(json.dumps(artifacts))
        return old, artifacts

    def test_archive_is_the_source_of_truth_even_without_legacy_stamp(self):
        before = wx.verify_source(self.archive, self.source)
        self.assertEqual(before, wx.verify_source(self.archive, self.source))
        (self.source / "code.cpp").write_text("int changed_source;\n")
        with self.assertRaisesRegex(RuntimeError, "differs from verified archive"):
            wx.verify_source(self.archive, self.source)

    def test_archive_tampering_is_rejected(self):
        with self.archive.open("ab") as stream: stream.write(b"mutated cache")
        with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            wx.verify_source(self.archive, self.source)

    def test_symlink_source_substitution_is_rejected(self):
        outside = self.root / "outside.cpp"; outside.write_bytes((self.source / "code.cpp").read_bytes())
        (self.source / "code.cpp").unlink(); (self.source / "code.cpp").symlink_to(outside)
        with self.assertRaisesRegex(RuntimeError, "differs from verified archive"):
            wx.verify_source(self.archive, self.source)

    def test_configure_executable_mode_is_bound(self):
        (self.source / "configure").chmod(0o644)
        with self.assertRaisesRegex(RuntimeError, "differs from verified archive"):
            wx.verify_source(self.archive, self.source)

    def test_out_of_tree_outputs_are_ignored_but_extra_source_is_not(self):
        original = wx.verify_source(self.archive, self.source)
        build = self.root / "build"; build.mkdir(); (build / "generated.o").write_bytes(b"output")
        self.assertEqual(original, wx.verify_source(self.archive, self.source))
        (self.source / "old-revision.cpp").write_text("unarchived source")
        with self.assertRaisesRegex(RuntimeError, "unarchived/missing files"):
            wx.verify_source(self.archive, self.source)

    def test_changed_compiler_or_configuration_rejects_cache(self):
        receipt = self.receipt()
        for key in ("compiler", "flags", "source"):
            changed = {**self.inputs, key: "changed"}
            with self.assertRaisesRegex(RuntimeError, "source/configuration/compiler changed"):
                wx.validate_receipt(receipt, changed, self.prefix)

    def test_installed_header_or_archive_mutation_rejects_cache(self):
        receipt = self.receipt()
        for path in (self.prefix / "header.h", self.prefix / "lib/libexample.a"):
            original = path.read_bytes(); path.write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "installation headers/static libraries/configuration changed"):
                wx.validate_receipt(receipt, self.inputs, self.prefix)
            path.write_bytes(original)

    def test_extra_installed_file_rejects_cache(self):
        receipt = self.receipt(); (self.prefix / "unexpected.a").write_bytes(b"unrecorded")
        with self.assertRaisesRegex(RuntimeError, "installation headers/static libraries/configuration changed"):
            wx.validate_receipt(receipt, self.inputs, self.prefix)

    def test_installed_symlink_cannot_escape_prefix(self):
        outside = self.root / "outside"; outside.write_text("outside")
        (self.prefix / "escaped").symlink_to(outside)
        with self.assertRaisesRegex(RuntimeError, "escapes its prefix"):
            wx.inventory(self.prefix)

    def test_legacy_experiment_adoption_requires_all_existing_evidence(self):
        self.legacy_evidence()
        proof = wx.adopt_experiment(self.root, self.inputs, self.prefix)
        self.assertEqual(proof["method"], "verified-experiment-adoption")
        self.assertFalse(proof["historical_compiler_binary_hash_recorded"])
        (self.prefix / "header.h").write_text("tampered")
        with self.assertRaisesRegex(RuntimeError, "artifacts differ from verified inventory"):
            wx.adopt_experiment(self.root, self.inputs, self.prefix)

    def test_legacy_config_or_compiler_is_not_blindly_relabelled(self):
        old, _ = self.legacy_evidence(); old["compiler"] = "different compiler"
        (self.root / "minimal-build-spec.json").write_text(json.dumps(old))
        with self.assertRaisesRegex(RuntimeError, "compiler/flags cannot be verified"):
            wx.adopt_experiment(self.root, self.inputs, self.prefix)

    def test_legacy_unrecorded_symlink_cannot_select_another_config(self):
        self.legacy_evidence()
        link = self.prefix / "bin/wx-config"; link.unlink(); link.symlink_to(self.prefix / "header.h")
        with self.assertRaisesRegex(RuntimeError, "unexpected/unverified symlinks"):
            wx.adopt_experiment(self.root, self.inputs, self.prefix)

    def test_linux_profile_refuses_other_platforms_before_building(self):
        with mock.patch.object(wx.sys, "platform", "darwin"):
            with self.assertRaisesRegex(RuntimeError, "Linux-only"):
                wx.build(self.root / "must-not-exist")
        self.assertFalse((self.root / "must-not-exist").exists())

    def test_partial_build_requires_a_fresh_cache_instead_of_new_receipt(self):
        # An existing prefix without historical manifests cannot be stamped
        # as if a verified successful build had just occurred.
        with self.assertRaisesRegex(RuntimeError, "lacks verified build evidence"):
            wx.adopt_experiment(self.root, self.inputs, self.prefix)


if __name__ == "__main__":
    unittest.main()
