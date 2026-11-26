"""Reject stale, incomplete or differently built package-suite receipts."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("macos_package", Path(__file__).parents[1] / "test-macos-package.py")
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class PackageReceiptTests(unittest.TestCase):
    def test_artifact_and_required_lifecycle_are_bound_without_a_stale_count(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "lurker"
            binary.write_bytes(b"candidate")
            names = ["create", "no-overwrite", "unlock", "application-io", "duplicate",
                     "aliased-duplicate", "busy", "close", "wrong-password", "reopen", "persistence", "reclose"]
            detail = {"status": "passed", "source_commit": "commit", "binary": str(binary),
                      "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                      "cases": [{"name": name, "status": "passed"} for name in names],
                      "counts": {"passed": 12, "failed": 0, "skipped": 0}}
            receipt = root / "result.json"
            receipt.write_text(json.dumps(detail))
            summary = {"status": "passed", "result": str(receipt), "counts": detail["counts"]}
            self.assertEqual(package.verify_suite(summary, binary, "commit", None, False), 12)
            with self.assertRaises(AssertionError):
                package.verify_suite(summary, binary, "commit", root, False)
            detail["cases"] = [case for case in detail["cases"] if case["name"] != "busy"]
            detail["counts"]["passed"] = 11
            receipt.write_text(json.dumps(detail))
            with self.assertRaisesRegex(AssertionError, "busy"):
                package.verify_suite(summary, binary, "commit", None, False)
            detail["cases"].append({"name": "busy", "status": "passed"})
            detail["counts"]["passed"] = 12
            receipt.write_text(json.dumps(detail))
            binary.write_bytes(b"different")
            with self.assertRaises(AssertionError):
                package.verify_suite(summary, binary, "commit", None, False)


if __name__ == "__main__":
    unittest.main()
