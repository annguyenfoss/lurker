"""Release signing with a disposable OpenPGP signing subkey."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "release-signatures.py"


@unittest.skipUnless(all(shutil.which(tool) for tool in ("gpg", "gpgv", "gpgconf")),
                     "requires GnuPG")
class ReleaseSignatures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temporary = tempfile.TemporaryDirectory(prefix="lurker-signing-")
        cls.addClassCleanup(temporary.cleanup)
        root = Path(temporary.name)
        home = root / "gpg"
        home.mkdir(mode=0o700)
        cls.env = {**os.environ, "GNUPGHOME": str(home)}
        cls.addClassCleanup(subprocess.run, ["gpgconf", "--kill", "gpg-agent"],
                            env=cls.env, check=False)
        gpg = ["gpg", "--no-options", "--batch", "--pinentry-mode", "loopback", "--passphrase", ""]

        def run(*args):
            return subprocess.run([*gpg, *args], env=cls.env, capture_output=True, check=True).stdout

        def fingerprints():
            return [line.split(b":")[9].decode("ascii") for line in run("--with-colons", "--list-keys").splitlines()
                    if line.startswith(b"fpr:")]

        run("--quick-gen-key", "Lurker fixture <release@example.invalid>", "ed25519", "cert", "1d")
        primary, = fingerprints()
        run("--quick-add-key", primary, "ed25519", "sign", "1d")
        cls.primary, cls.signer = fingerprints()
        cls.keyring = root / "publisher.gpg"
        cls.keyring.write_bytes(run("--export", primary))

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="lurker-release-")
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.manifest = self.directory / "SHA256SUMS"
        self.signature = self.directory / "SHA256SUMS.sig"
        self.package = self.directory / "lurker.deb"
        self.source = self.directory / "lurker-src.tar.gz"
        self.package.write_bytes(b"package fixture")
        self.source.write_bytes(b"source fixture")
        self.manifest.write_text("".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n"
            for path in (self.package, self.source)
        ))

    def invoke(self, operation, *options, signer=None):
        command = [sys.executable, str(SCRIPT), operation, str(self.directory),
                   "--signer", signer or self.signer, *options]
        if operation == "verify":
            command.extend(("--keyring", str(self.keyring)))
        return subprocess.run(command, check=False, capture_output=True, text=True, env=self.env, timeout=30)

    def sign(self):
        result = self.invoke("sign")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_sign_and_verify_with_exact_subkey_and_artifact_set(self):
        self.sign()
        verified = self.invoke("verify")
        self.assertEqual(verified.returncode, 0, verified.stderr)
        self.assertEqual(set(json.loads(verified.stdout)["artifacts"]), {self.package.name, self.source.name})
        self.source.unlink()
        selected = self.invoke("verify", "--artifact", self.package.name)
        self.assertEqual(selected.returncode, 0, selected.stderr)
        self.assertEqual(set(json.loads(selected.stdout)["artifacts"]), {self.package.name})
        self.assertNotEqual(self.invoke("verify", signer=self.primary).returncode, 0)
        self.assertNotEqual(self.invoke("verify", "--artifact", "unlisted.deb").returncode, 0)
        self.assertNotEqual(self.invoke("verify").returncode, 0)

    def test_tampering_and_replacing_an_existing_signature_are_refused(self):
        self.sign()
        original_signature = self.signature.read_bytes()
        self.assertNotEqual(self.invoke("sign").returncode, 0)
        self.assertEqual(self.signature.read_bytes(), original_signature)
        package = self.package.read_bytes()
        self.package.write_bytes(b"modified package")
        self.assertIn("Artifact checksum differs", self.invoke("verify").stderr)
        self.package.write_bytes(package)
        self.manifest.write_bytes(self.manifest.read_bytes() + b"\n")
        self.assertNotEqual(self.invoke("verify").returncode, 0)

    def test_signer_rejects_altered_or_ambiguous_inputs(self):
        expected = self.manifest.read_bytes()
        self.package.write_bytes(b"modified package")
        self.assertIn("Artifact checksum differs", self.invoke("sign").stderr)
        for manifest in (expected.replace(b"lurker.deb", b"../lurker.deb"), expected + expected):
            self.manifest.write_bytes(manifest)
            self.assertNotEqual(self.invoke("sign").returncode, 0)
        self.manifest.write_bytes(expected)
        self.package.unlink()
        self.package.symlink_to(self.source)
        self.assertNotEqual(self.invoke("sign").returncode, 0)
        self.assertFalse(self.signature.exists())


if __name__ == "__main__":
    unittest.main()
