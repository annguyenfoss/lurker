"""Verified staging archive input for the native package builders."""

import hashlib
import importlib.util
import os
from pathlib import Path
import re
import tarfile


SPEC = importlib.util.spec_from_file_location("stage_unix", Path(__file__).with_name("stage-unix.py"))
STAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STAGE)


def verify_rust_notices(prefix, commit, files):
    provenance_path = "share/lurker/provenance/rust-binaries.json"
    notices_path = "share/lurker/licenses/rust"
    if provenance_path not in files or notices_path + "/packages.json" not in files:
        raise ValueError("Native packages require Rust dependency notices recorded with --cargo-log")
    provenance = STAGE.json_file(prefix / provenance_path)
    notices = provenance.get("rust_notices", {})
    if (provenance.get("source_commit") != commit or provenance.get("backend") != "binaries"
            or not isinstance(notices, dict)
            or not re.fullmatch(r"[0-9a-f]{64}", str(notices.get("cargo_log_sha256", "")))):
        raise ValueError("Rust notice provenance does not match the staged build")
    notice_prefix = notices_path + "/"
    inventory = {name.removeprefix(notice_prefix): entry
                 for name, entry in files.items() if name.startswith(notice_prefix)}
    if inventory != notices.get("files"):
        raise ValueError("Staged Rust notices differ from binary provenance")
    catalog = STAGE.json_file(prefix / notices_path / "packages.json")
    products = provenance.get("files")
    packages = catalog.get("packages")
    if (not isinstance(products, dict) or not products or catalog.get("source_commit") != commit
            or catalog.get("products") != sorted(products) or not isinstance(packages, list) or not packages):
        raise ValueError("Rust notice catalog does not match the recorded products")
    for name, entry in files.items():
        binary = Path(name)
        if binary.parent == Path("bin") and "symlink" not in entry:
            if not entry.get("executable") or entry["sha256"] != products.get(binary.name, {}).get("sha256"):
                raise ValueError("Staged Rust product differs from the recorded build: " + name)


def unpack_archive(archive, sha256, commit, scratch):
    with archive.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != sha256:
            raise ValueError("Staged archive SHA-256 differs")
        stream.seek(0)
        with tarfile.open(fileobj=stream, mode="r:gz") as source:
            entries = {}
            for member in source.getmembers():
                name = STAGE.relative(member.name)
                if name.parts[0] != "lurker" or name in entries:
                    raise ValueError("Unexpected or duplicate archive entry: " + member.name)
                if not (member.isdir() or member.isfile() or member.issym()):
                    raise ValueError("Unsupported archive entry: " + member.name)
                if not member.issym() and member.mode & 0o7000:
                    raise ValueError("Unsafe mode on archive entry: " + member.name)
                entries[name] = member
            for name in entries:
                if any(parent in entries and not entries[parent].isdir() for parent in name.parents):
                    raise ValueError("Archive member has a non-directory ancestor: " + str(name))
            source.extractall(scratch, filter="data")
    prefix = STAGE.safe_root(scratch / "lurker")
    metadata = "share/lurker/manifest.json"
    manifest = STAGE.json_file(prefix / metadata)
    STAGE.verify_platform(manifest)
    if manifest.get("schema") != 1 or manifest.get("source_commit") != commit:
        raise ValueError("Staged manifest does not match the requested source")
    files = manifest["files"]
    if STAGE.file_manifest(prefix, exclude=(metadata,)) != files:
        raise ValueError("Staged file inventory differs from manifest")
    for name, entry in files.items():
        if "symlink" in entry and os.readlink(prefix / name) != entry["symlink"]:
            raise ValueError("Staged symlink target differs from manifest: " + name)
    verify_rust_notices(prefix, commit, files)
    return prefix, manifest
