# Native OpenBSD LKL diagnostic

This work produces an external host-tool candidate. It does not install a
filesystem driver, mount LKL, modify the kernel or enable a Lurker filesystem.
Read `DIRECTIVES/PLAN/NATIVE_FRESH_20261127/OPENBSD_LKL_ASSESSMENT.md` first.

The first diagnostic preserves unmodified upstream inputs at
`d0f76a77e8c2ca68bd7d96190b8da09040eab072`, archive SHA-256
`ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168`.
Use a work-specific native build filesystem with at least 6 GiB free, as an
ordinary OpenBSD amd64 user:

```sh
python3 scripts/tests/test_build_openbsd_lkl.py
python3 scripts/build-openbsd-lkl.py --work-dir /path/to/lkl-work \
  --receipt /path/to/lkl-work/baseline.json --timeout 600
```

The one-worker command builds only the library by default. `--target boot`
builds the upstream RAM boot test; it does not execute it. Every original source
input is checked against the pinned archive before reuse. Existing receipts,
logs, changed sources and failed downloads are preserved. A deadline terminates
the entire newly owned process group, including descendants that ignore SIGTERM
after their parent exits.

Native FUSE ABI, timers, stack/mapping operations, generated UAPI, authenticated
request credentials, Linux error translation, synchronous namespace publication,
checked backing flush and normal teardown remain separate required gates.
Compilation or RAM boot success never authorizes a filesystem mount. Existing
installed providers, old scratch devices and failed fixtures remain untouched.
