"""Pure packaging checks; no crypto/filesystem backend is executed."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("cryptomator_build", Path(__file__).resolve().parents[1] / "build-cryptomator.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class DependencyLockTests(unittest.TestCase):
    def test_missing_extra_and_modified_jars_fail_closed(self):
        with tempfile.TemporaryDirectory(prefix="lurker-cryptomator-lock-") as directory:
            root = Path(directory)
            modules = root / "mods"
            modules.mkdir()
            jar = modules / "backend.jar"
            jar.write_bytes(b"reviewed artifact")
            (root / "dependencies.lock.json").write_text(json.dumps({"sha256": {jar.name: BUILD.digest(jar)}}))
            with patch.object(BUILD, "SOURCE", root):
                BUILD.verify_dependencies(modules)
                jar.write_bytes(b"changed artifact")
                with self.assertRaises(SystemExit):
                    BUILD.verify_dependencies(modules)
                jar.unlink()
                with self.assertRaises(SystemExit):
                    BUILD.verify_dependencies(modules)
                jar.write_bytes(b"reviewed artifact")
                (modules / "unreviewed.jar").write_bytes(b"unreviewed")
                with self.assertRaises(SystemExit):
                    BUILD.verify_dependencies(modules)


if __name__ == "__main__":
    unittest.main()
