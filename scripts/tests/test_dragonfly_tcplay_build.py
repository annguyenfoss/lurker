"""A physical-source bundle requires the measured native wrapper extension."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "build-dragonfly-tcplay.py"
spec = importlib.util.spec_from_file_location("dragonfly_tcplay_builder", SCRIPT)
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class PhysicalSourceProvenance(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="lurker-tcplay-build-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.native = self.root / "native"
        self.native.mkdir()
        self.cache = self.root / "cache"
        source = self.cache / "pinned/input.c"
        source.parent.mkdir(parents=True)
        source.write_text("/* Copyright native fixture */\n#include <stdio.h>\n")
        pin = {"revision": "pinned", "sources": ["input.c"],
               "sha256": {"input.c": builder.digest(source)}}
        (self.native / "sources.json").write_text(json.dumps(pin))
        (self.native / "wrapper.c").write_text("reviewed physical wrapper fixture\n")
        (self.native / "lurker.patch").write_text("reviewed patch fixture\n")
        self.output = self.root / "output"

    def fake_compile(self, arguments, **kwargs):
        if arguments[0] == "/usr/bin/cc":
            Path(arguments[arguments.index("-o") + 1]).write_bytes(b"native wrapper artifact fixture")

    def test_physical_attestation_requires_the_actual_extension_handshake(self):
        with patch.object(builder, "NATIVE", self.native), \
             patch.object(builder.subprocess, "run", side_effect=self.fake_compile), \
             patch.object(builder.subprocess, "check_output",
                          side_effect=["lurker-tcplay 2\n", "lurker-tcplay physical 1\n"]) as query:
            builder.build(self.cache, self.output, True)
        self.assertEqual([call.args[0][-1] for call in query.call_args_list],
                         ["protocol", "physical-protocol"])
        manifest = json.loads((self.output / "dragonfly-tcplay-source.json").read_text())
        self.assertEqual(manifest["protocol"], "lurker-tcplay 2")
        self.assertEqual(manifest["runtime"], {"physical_source_protocol": 1})
        self.assertEqual(manifest["inputs"]["wrapper.c"], builder.digest(self.native / "wrapper.c"))
        self.assertEqual(manifest["files"][builder.BINARY],
                         hashlib.sha256((self.output / builder.BINARY).read_bytes()).hexdigest())

    def test_old_or_unqualified_wrapper_cannot_publish_a_physical_bundle(self):
        with patch.object(builder, "NATIVE", self.native), \
             patch.object(builder.subprocess, "run", side_effect=self.fake_compile), \
             patch.object(builder.subprocess, "check_output",
                          side_effect=["lurker-tcplay 2\n", "unqualified physical extension\n"]):
            with self.assertRaisesRegex(SystemExit, "physical wrapper protocol"):
                builder.build(self.cache, self.output, True)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
