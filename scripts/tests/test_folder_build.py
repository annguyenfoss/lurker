import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("folder_build", Path(__file__).parents[1] / "build-folder-backends.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class NoticeTests(unittest.TestCase):
    def test_rebuild_replaces_readonly_module_cache_notices(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, output = root / "source", root / "LICENSE"
            source.write_text("updated notice")
            source.chmod(0o444)
            output.write_text("old notice")
            output.chmod(0o444)
            BUILD.copy_notice(source, output)
            BUILD.copy_notice(source, output)
            self.assertEqual(output.read_text(), "updated notice")
            self.assertEqual(output.stat().st_mode & 0o777, 0o644)
            self.assertFalse(list(root.glob(".notice-*")))

    def test_runtime_inventory_retains_vendor_patents_without_build_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            goroot, output = root / "toolchain", root / "notices"
            vendor = goroot / "src/vendor/example.org/crypto"
            vendor.mkdir(parents=True)
            (goroot / "LICENSE").write_text("Go BSD notice")
            (vendor / "PATENTS").write_text("patent grant")
            (vendor / "LICENSE").write_text("dependency notice")
            with patch.object(BUILD, "capture", return_value=str(goroot)):
                inventory = BUILD.go_runtime_notices(root, output)
            self.assertIn("LICENSE", inventory)
            self.assertIn("src/vendor/example.org/crypto/PATENTS", inventory)
            self.assertTrue(all(not Path(path).is_absolute() for path in inventory))


if __name__ == "__main__":
    unittest.main()
