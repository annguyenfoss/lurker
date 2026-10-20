#!/usr/bin/env python3
"""Build a native release app and a drag-to-Applications DMG using Apple tools."""
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(*arguments, capture=False):
    return subprocess.run(
        [str(arg) for arg in arguments], cwd=ROOT, check=True,
        text=True, stdout=subprocess.PIPE if capture else None,
    ).stdout


def layout_tools():
    try:
        from ds_store import DSStore
        from mac_alias import Alias
        return DSStore, Alias
    except ImportError:
        interpreter = ROOT / "build/macos-dmg-tools/bin/python"
        if interpreter.exists() and Path(sys.prefix).resolve() != interpreter.parent.parent.resolve():
            os.execv(str(interpreter), [str(interpreter), str(Path(__file__).resolve()), *sys.argv[1:]])
        raise SystemExit(
            "Install DMG build dependencies first:\n"
            "python3 -m venv build/macos-dmg-tools\n"
            "build/macos-dmg-tools/bin/python -m pip install -r scripts/requirements-macos-dmg.txt"
        )


def configure_finder(mountpoint, DSStore, Alias):
    # Author the alias on the mounted HFS+ volume so it refers to the image's
    # volume and file IDs, not a temporary folder on the developer's disk.
    background = mountpoint / ".background/background.tiff"
    alias = Alias.for_file(str(background))
    with DSStore.open(str(mountpoint / ".DS_Store"), "w+") as store:
        store["."]["vSrn"] = ("long", 1)
        store["."]["icvl"] = ("type", b"icnv")
        store["."]["vstl"] = ("type", b"icnv")
        store["."]["bwsp"] = {
            "WindowBounds": "{{240, 160}, {800, 528}}",
            "ShowStatusBar": False, "ShowToolbar": False,
            "ShowPathbar": False, "ShowSidebar": False,
            "ShowTabView": False, "ContainerShowSidebar": False,
            "PreviewPaneVisibility": False, "SidebarWidth": 0,
        }
        store["."]["icvp"] = {
            "viewOptionsVersion": 1, "backgroundType": 2,
            "backgroundImageAlias": alias.to_bytes(),
            "backgroundColorRed": 0.027, "backgroundColorGreen": 0.082,
            "backgroundColorBlue": 0.133, "arrangeBy": "none",
            "gridOffsetX": 0.0, "gridOffsetY": 0.0, "gridSpacing": 100.0,
            "scrollPositionX": 0.0, "scrollPositionY": 0.0,
            "showIconPreview": False, "showItemInfo": False,
            "labelOnBottom": True, "textSize": 14.0, "iconSize": 112.0,
        }
        store["Lurker.app"]["Iloc"] = (240, 348)
        store["Applications"]["Iloc"] = (560, 348)


def main():
    if sys.platform != "darwin":
        raise SystemExit("Build the macOS DMG on macOS with Rust and Xcode command-line tools.")
    DSStore, Alias = layout_tools()
    metadata = json.loads(run("cargo", "metadata", "--no-deps", "--format-version", "1", capture=True))
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "lurker-desktop")
    host = next(line.removeprefix("host: ") for line in run("rustc", "-vV", capture=True).splitlines() if line.startswith("host: "))
    architecture = {"aarch64-apple-darwin": "arm64", "x86_64-apple-darwin": "x86_64"}.get(host)
    if architecture is None:
        raise SystemExit(f"Unsupported native macOS build target: {host}")
    run("cargo", "build", "--release", "--locked", "--target", host, "-p", "lurker-desktop", "-p", "lurker-helper")
    binaries = Path(metadata["target_directory"]) / host / "release"
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    name = f"Lurker-{version}-macos-{architecture}.dmg"
    identity = os.environ.get("CODESIGN_IDENTITY", "-")

    with tempfile.TemporaryDirectory(prefix=".lurker-dmg-", dir=output) as temporary:
        working = Path(temporary)
        image = working / "image"
        app = image / "Lurker.app"
        executable_dir = app / "Contents/MacOS"
        resources = app / "Contents/Resources"
        executable_dir.mkdir(parents=True)
        resources.mkdir()
        minimum_versions = []
        for binary in ["lurker-desktop", "lurker-helper"]:
            source = binaries / binary
            # All Rust/UI assets are embedded. Reject an accidentally linked
            # Homebrew or developer-local dylib instead of shipping a broken app.
            for line in run("otool", "-L", source, capture=True).splitlines()[1:]:
                dependency = line.strip().split(" (", 1)[0]
                if not dependency.startswith(("/System/Library/", "/usr/lib/")):
                    raise SystemExit(f"Bundle this non-system dependency before packaging: {dependency}")
            build_info = run("xcrun", "vtool", "-show-build", source, capture=True)
            minimum_versions.extend(re.findall(r"\bminos\s+([\d.]+)", build_info))
            destination = executable_dir / binary
            shutil.copyfile(source, destination)
            destination.chmod(0o755)
            run("strip", "-x", destination)
        if not minimum_versions:
            raise SystemExit("Could not determine the binaries' macOS deployment target.")
        minimum = max(minimum_versions, key=lambda v: tuple(map(int, v.split("."))))

        iconset = working / "Lurker.iconset"
        iconset.mkdir()
        for size in [16, 32, 128, 256, 512]:
            for scale in [1, 2]:
                pixels = size * scale
                suffix = "@2x" if scale == 2 else ""
                run("sips", "-z", pixels, pixels, ROOT / "extras/macos/lurker-circle.png",
                    "--out", iconset / f"icon_{size}x{size}{suffix}.png", capture=True)
        run("iconutil", "-c", "icns", iconset, "-o", resources / "Lurker.icns")
        info = {
            "CFBundleName": "Lurker",
            "CFBundleDisplayName": "Lurker",
            "CFBundleIdentifier": "org.lurker.desktop",
            "CFBundleExecutable": "lurker-desktop",
            "CFBundleIconFile": "Lurker.icns",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version,
            "CFBundleVersion": version,
            "CFBundleInfoDictionaryVersion": "6.0",
            "LSMinimumSystemVersion": minimum,
            "NSHighResolutionCapable": True,
            "NSPrincipalClass": "NSApplication",
            "NSHumanReadableCopyright": "Licensed under GPL-3.0-only. See LICENSE.txt.",
        }
        with (app / "Contents/Info.plist").open("wb") as file:
            plistlib.dump(info, file)
        (app / "Contents/PkgInfo").write_bytes(b"APPL????")
        shutil.copyfile(ROOT / "LICENSE", resources / "LICENSE.txt")
        (resources / "LICENSE.txt").chmod(0o644)
        run("plutil", "-lint", app / "Contents/Info.plist")
        for target in [executable_dir / "lurker-helper", executable_dir / "lurker-desktop", app]:
            run("codesign", "--force", "--sign", identity, "--options", "runtime",
                "--timestamp=none" if identity == "-" else "--timestamp", target)
        run("codesign", "--verify", "--deep", "--strict", "--verbose=2", app)

        (image / "Applications").symlink_to("/Applications", target_is_directory=True)
        signature_note = (
            "This local build is ad-hoc signed and is not Apple-notarized.\n"
            if identity == "-" else "This build is signed but this script does not notarize it.\n"
        )
        background = image / ".background"
        background.mkdir()
        shutil.copyfile(ROOT / "extras/macos/dmg-background.tiff", background / "background.tiff")
        (background / "Install Lurker.txt").write_text(
            "Lurker\n\n"
            "Drag Lurker.app onto Applications, then eject this disk image.\n"
            "Open Lurker from Applications. No package installer or terminal install is needed.\n\n"
            "Apple encrypted disk images work with built-in macOS tools.\n"
            "VeraCrypt requires VeraCrypt and its matching FUSE runtime to be installed separately.\n\n"
            + signature_note + f"Requires macOS {minimum} or later; architecture: {architecture}.\n",
            encoding="utf-8",
        )
        writable_dmg = working / "layout.dmg"
        run("hdiutil", "create", "-volname", "Lurker", "-srcfolder", image,
            "-fs", "HFS+", "-format", "UDRW", writable_dmg)
        mountpoint = working / "mounted"
        run("hdiutil", "attach", "-readwrite", "-nobrowse", "-noautoopen",
            "-mountpoint", mountpoint, writable_dmg, capture=True)
        try:
            configure_finder(mountpoint, DSStore, Alias)
            run("codesign", "--verify", "--deep", "--strict", mountpoint / "Lurker.app")
        finally:
            run("hdiutil", "detach", mountpoint)
        staged_dmg = working / name
        run("hdiutil", "convert", writable_dmg, "-format", "UDZO", "-o", staged_dmg)
        run("hdiutil", "verify", staged_dmg)
        destination = output / name
        staged_dmg.replace(destination)
        checksum = hashlib.sha256(destination.read_bytes()).hexdigest()
        (output / f"{name}.sha256").write_text(f"{checksum}  {name}\n", encoding="ascii")
    print(f"\nDrag-and-drop installer: {destination}")
    print(f"Signing: {'ad-hoc; not notarized' if identity == '-' else identity}")


if __name__ == "__main__":
    main()
