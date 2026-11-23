#!/usr/bin/env python3
"""Reuse native CLI conformance with binary keyfile-only VeraCrypt credentials.

Requires an explicit CLI artifact and a dedicated test host. Fixtures stay in
the chosen private /tmp parent; failures retain evidence and never force-close.
"""
import importlib.util
import json
import os
from pathlib import Path
import sys

SPEC = importlib.util.spec_from_file_location("native_conformance", Path(__file__).with_name("test-backends.py"))
conformance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(conformance)


class KeyfileSuite(conformance.Suite):
    def __init__(self, options):
        super().__init__(options)
        self.keyfile = self.root / "key, 密碼 with spaces.bin"
        self.wrong_keyfile = self.root / "wrong key.bin"
        for path in [self.keyfile, self.wrong_keyfile]:
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(descriptor, "wb") as stream:
                stream.write(b"\0\xff\n\r" + os.urandom(252))
        self.result["authentication"] = "binary_keyfile_only"
        self.result["limitations"] = [item.replace("keyfile, ", "") for item in self.result["limitations"]]
        self.save()

    def cli(self, args, expected=True, password=None):
        # Reuse every lifecycle case, including direct alias/duplicate calls.
        # The superclass's wrong-credential marker selects the wrong binary
        # key; it is never sent to the CLI or to a password prompt.
        arguments = list(args)
        if arguments[2] in {"create", "mount"}:
            key = self.wrong_keyfile if password == self.wrong else self.keyfile
            arguments.extend(["--key-file", str(key)])
        result = super().cli(arguments, expected=expected, password=None)
        if result["prompts"]:
            raise AssertionError("Keyfile-only authentication unexpectedly prompted for a password")
        return result

    def case(self, name, function=None, skip=None):
        return super().case("wrong-keyfile" if name == "wrong-password" else name, function, skip)


def main(argv=None):
    try:
        options = conformance.parse_args(argv)
        if options.backend != "veracrypt":
            raise ValueError("This suite requires --backend veracrypt")
        suite = KeyfileSuite(options)
        result = suite.run()
        print(json.dumps({"schema": 1, "status": result["status"], "backend": "veracrypt",
                          "authentication": result["authentication"], "counts": result["counts"],
                          "duration_seconds": result["duration_seconds"],
                          "result": str(suite.root / "result.json"), "diagnostic": result.get("diagnostic")}))
        return 0 if result["status"] == "passed" else 1
    except (OSError, ValueError) as error:
        print(json.dumps({"schema": 1, "status": "blocked", "diagnostic": str(error)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
