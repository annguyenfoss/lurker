#!/usr/bin/env python3
"""Use ordinary installed openSUSE tools through an administrator-owned harness."""
import os,runpy
from pathlib import Path
os.environ["PATH"]="/usr/local/sbin:/usr/local/bin:/usr/sbin:/sbin:/usr/bin:/bin"
runpy.run_path(str(Path(__file__).with_name("test-filesystem-cli-variations.py")),run_name="__main__")
