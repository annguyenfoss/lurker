#!/usr/bin/env python3
"""Round-trip a single PDF through the built Lurker file encryption CLI."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import secrets
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()
    cli = (args.binary or Path(os.environ["CARGO_TARGET_DIR"]) / "debug/lurker").resolve(strict=True)
    evidence = Path(os.environ.get("LURKER_TEST_JOB_DIR", ".")).resolve()
    logs = evidence / "file-encryption-logs"
    logs.mkdir(mode=0o700, exist_ok=True)
    helpers = runpy.run_path(str(Path(__file__).with_name("test-filesystem-cli.py")))
    pdf = helpers["sample_pdf"]()
    os.environ["LC_ALL"] = "C"
    if sys.platform == "darwin" and Path("/opt/homebrew/bin").is_dir():
        os.environ["PATH"] += ":/opt/homebrew/bin"
    gpg = shutil.which("gpg")
    if gpg is None:
        raise RuntimeError("GnuPG executable is missing from PATH")
    openssl = find_openssl()
    versions = {"gpg": tool_version(gpg, "--version"), "openssl": tool_version(openssl, "version")}
    results = []
    with tempfile.TemporaryDirectory(prefix="lurker-file-crypto-", dir="/tmp") as directory:
        run_cases(Path(directory), cli, gpg, openssl, pdf, logs, helpers["terminal"], results)
    receipt = {
        "commit": os.environ.get("LURKER_TEST_SOURCE_COMMIT"),
        "binary_sha256": helpers["file_hash"](cli),
        "pdf_sha256": hashlib.sha256(pdf).hexdigest(),
        "tools": versions,
        "cases": results,
    }
    helpers["atomic_json"](evidence / "file-encryption-receipt.json", receipt)
    print(json.dumps(receipt, sort_keys=True))


def tool_version(executable, arg):
    result = subprocess.check_output([executable, arg], text=True, timeout=15)
    return {"path": executable, "version": result.splitlines()[0]}


def find_openssl():
    for candidate in (
        shutil.which("openssl"),
        shutil.which("eopenssl"),
        "/opt/homebrew/opt/openssl@3/bin/openssl",
        "/usr/local/bin/openssl",
        "/usr/pkg/bin/openssl",
    ):
        if candidate and os.access(candidate, os.X_OK):
            if tool_version(candidate, "version")["version"].startswith("OpenSSL "):
                return candidate
    raise RuntimeError("A compatible OpenSSL executable is missing")


def run_cases(directory, cli, gpg, openssl, pdf, logs, terminal, results):
    source = directory / "sample.pdf"
    source.write_bytes(pdf)
    passphrase = secrets.token_urlsafe(32)
    passphrase_file = directory / "passphrase.key"
    passphrase_file.write_text(passphrase + "\nunused second line\n")
    shared_key = directory / "shared.key"
    shared_key.write_bytes(secrets.token_bytes(32))
    for path in (passphrase_file, shared_key):
        path.chmod(0o600)

    def fixture(executable, *argv):
        return subprocess.run([executable, *map(str, argv)], cwd=directory, capture_output=True,
                              timeout=60, check=True).stdout

    def invoke(label, *argv, password=None, fail=False):
        command = [str(cli), *map(str, argv)]
        log = logs / f"{label}.log"
        if password is not None:
            outcome = terminal(command, password, 60, log)
            if outcome["timed_out"] or outcome["prompts"] == 0:
                raise RuntimeError(f"{label}: passphrase prompt failed. See {log}")
            exit_code = outcome["exit_code"]
        else:
            completed = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, timeout=60)
            log.write_bytes(completed.stdout + completed.stderr)
            exit_code = completed.returncode
        if (exit_code != 0) != fail:
            raise RuntimeError(f"{label}: unexpected exit {exit_code}. See {log}")
        return log

    def case(label, backend, encrypt_flags, routes):
        encrypted = directory / f"{label}.encrypted"
        restore = directory / f"{label}.restored.pdf"
        invoke(label + "-encrypt", "encrypt", "--type", backend, *encrypt_flags,
               source, encrypted, password=passphrase if "--passphrase" in encrypt_flags else None)
        if encrypted.read_bytes() in (b"", pdf):
            raise RuntimeError(f"{label}: expected nonempty ciphertext")
        for number, (flags, home) in enumerate(routes, start=1):
            if home is not None:
                os.environ["GNUPGHOME"] = str(home)
            invoke(f"{label}-decrypt-{number}", "decrypt", "--type", backend, *flags,
                   encrypted, restore, password=passphrase if "--passphrase" in flags else None)
            if restore.read_bytes() != pdf:
                raise RuntimeError(f"{label}: restored PDF differs from the input")
            restore.unlink()
        results.append({"case": label, "round_trips": len(routes)})
        return encrypted

    def reject(label, backend, ciphertext, *flags, password=None):
        output = directory / "rejected.pdf"
        invoke(label, "decrypt", "--type", backend, *flags, ciphertext, output,
               password=password, fail=True)
        if output.exists() or list(directory.glob(".lurker-file-*")):
            raise RuntimeError(f"{label}: failed decryption left output")
        results.append({"case": label, "rejected": True})

    age_password = case("age-passphrase", "age", ["--passphrase"], [(["--passphrase"], None)])
    age_public = []
    age_private = []
    for person in ("Alice", "Bob"):
        identity = directory / f"{person}.ssh"
        fixture("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", identity)
        age_public.extend(["--public-key", identity.with_suffix(".ssh.pub")])
        age_private.append((["--private-key", identity], None))
    case("age-recipients", "age", age_public, age_private)
    reject("age-wrong-passphrase", "age", age_password, "--passphrase", password="wrong-" + passphrase)

    homes = []
    gpg_public = []
    fingerprints = []
    try:
        for person in ("Alice", "Bob"):
            home = directory / f"gpg-{person}"
            home.mkdir(mode=0o700)
            homes.append(home)
            os.environ["GNUPGHOME"] = str(home)
            fixture(gpg, "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                    "--quick-generate-key", f"{person} <{person.lower()}@lurker.test>", "ed25519", "cert", "0")
            keys = fixture(gpg, "--batch", "--with-colons", "--list-keys").decode().splitlines()
            fingerprint = next(line.split(":")[9] for line in keys if line.startswith("fpr:"))
            fingerprints.append(fingerprint)
            fixture(gpg, "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                    "--quick-add-key", fingerprint, "cv25519", "encr", "0")
            public = directory / f"{person}.asc"
            public.write_bytes(fixture(gpg, "--batch", "--armor", "--export", fingerprint))
            gpg_public.extend(["--public-key", public])

        os.environ["GNUPGHOME"] = str(homes[0])
        listed = invoke("gnupg-keys", "keys", "--type", "gnupg").read_text()
        if fingerprints[0] not in listed:
            raise RuntimeError("GnuPG key listing did not contain Alice's fingerprint")
        case("gnupg-keyring", "gnupg", ["--public-key", fingerprints[0]], [([], homes[0])])
        case("gnupg-passphrase", "gnupg", ["--passphrase"], [(["--passphrase"], None)])
        key_flags = ["--key-file", passphrase_file]
        case("gnupg-passphrase-file", "gnupg", key_flags, [(["--passphrase"], None), (key_flags, None)])
        case("gnupg-public-files", "gnupg", gpg_public, [([], home) for home in homes])
        mixed = case("gnupg-mixed", "gnupg", gpg_public + key_flags,
                     [([], home) for home in homes] + [(key_flags, None)])
        reject("gnupg-wrong-passphrase", "gnupg", mixed, "--passphrase", password="wrong-" + passphrase)
    finally:
        gpgconf = Path(gpg).with_name("gpgconf")
        for home in homes:
            subprocess.run([gpgconf, "--homedir", home, "--kill", "all"], capture_output=True, timeout=15)
        os.environ.pop("GNUPGHOME", None)

    cms_public = []
    cms_private = []
    for person in ("Alice", "Bob"):
        keyfile = directory / f"{person}.key"
        certificate = directory / f"{person}.crt"
        fixture(openssl, "req", "-new", "-x509", "-newkey", "rsa:2048", "-noenc",
                "-subj", f"/CN={person}", "-keyout", keyfile, "-out", certificate)
        cms_public.extend(["--public-key", certificate])
        cms_private.append((["--private-key", keyfile, "--certificate", certificate], None))
    cms_key = ["--key-file", shared_key]
    cms_password = ["--passphrase"]
    case("cms-certificates", "openssl", cms_public, cms_private)
    case("cms-shared-key", "openssl", cms_key, [(cms_key, None)])
    case("cms-passphrase", "openssl", cms_password, [(cms_password, None)])
    mixed = case("cms-mixed", "openssl", cms_public + cms_key + cms_password,
                 cms_private + [(cms_key, None), (cms_password, None)])
    reject("cms-wrong-passphrase", "openssl", mixed, "--passphrase", password="wrong-" + passphrase)


if __name__ == "__main__":
    main()
