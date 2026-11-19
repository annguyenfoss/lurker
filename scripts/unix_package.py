"""Verified staging archive input for the native package builders."""

import hashlib
import importlib.util
import os
from pathlib import Path
import tarfile


SPEC = importlib.util.spec_from_file_location("stage_unix", Path(__file__).with_name("stage-unix.py"))
STAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STAGE)


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
    return prefix, manifest
