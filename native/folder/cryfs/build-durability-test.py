#!/usr/bin/env python3
"""Compile (without executing) the Linux CryFS durability regression.

Uses the compiler, compile flags and static dependency closure of an existing
patched Conan/Unix Makefiles Release build. Runtime tests belong on a dedicated
native host, never endeavour.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import shlex
import subprocess


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="Patched CryFS source with build/Release")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    build = args.source.resolve() / "build/Release"
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    flags = (build / "src/blockstore/CMakeFiles/blockstore.dir/flags.make").read_text().splitlines()
    options = []
    for key in ("CXX_DEFINES = ", "CXX_INCLUDES = ", "CXX_FLAGS = "):
        options.extend(shlex.split(next(line[len(key):] for line in flags if line.startswith(key))))
    cache = (build / "CMakeCache.txt").read_text().splitlines()
    compiler = next(line.partition("=")[2] for line in cache if line.startswith("CMAKE_CXX_COMPILER:FILEPATH="))
    obj = output.with_suffix(".o")
    source = Path(__file__).with_name("durability-test.cpp").resolve()
    subprocess.run([compiler, *options, "-c", str(source), "-o", str(obj)], check=True)
    link = shlex.split((build / "src/cryfs-cli/CMakeFiles/cryfs-cli_bin.dir/link.txt").read_text())
    link[link.index("CMakeFiles/cryfs-cli_bin.dir/main.cpp.o")] = str(obj)
    link[link.index("-o") + 1] = str(output)
    subprocess.run(link, cwd=build / "src/cryfs-cli", check=True)


if __name__ == "__main__":
    main()
