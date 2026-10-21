#!/usr/bin/env bash
# Build the current worktree, including uncommitted source changes, without
# modifying the user's Git checkout or publishing anything.
set -euo pipefail
lurker_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
mkdir -p "$lurker_root/dist"
lurker_output=$(mktemp -d "$lurker_root/dist/aur-build-XXXXXXXX")
mkdir -p "$lurker_output/src"
git clone --quiet --no-hardlinks --local "$lurker_root" "$lurker_output/src/lurker"
python - "$lurker_root" "$lurker_output/src/lurker" <<'PY'
import pathlib
import shutil
import subprocess
import sys
root, destination = map(pathlib.Path, sys.argv[1:])
files = subprocess.check_output(['git', '-C', str(root), 'ls-files', '-z', '--cached', '--others', '--exclude-standard']).split(b'\0')
for raw in files:
    if not raw:
        continue
    relative = pathlib.Path(raw.decode())
    source, target = root / relative, destination / relative
    if source.is_file():
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    elif not source.exists() and target.is_file():
        target.unlink()
PY
cp "$lurker_root/packaging/aur/PKGBUILD" "$lurker_output/PKGBUILD"
# Every makepkg destination stays in this fresh local directory.
export PKGDEST="$lurker_output" SRCDEST="$lurker_output" SRCPKGDEST="$lurker_output"
export LOGDEST="$lurker_output" BUILDDIR="$lurker_output"
cd "$lurker_output"
# --noextract deliberately builds the worktree snapshot above. No Git fetch.
makepkg --noextract --force --log
makepkg --printsrcinfo > .SRCINFO
printf '\nLocal package and build logs: %s\n' "$lurker_output"
