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

The first unmodified source diagnostic on the base compiler/linker stopped in
configuration: native LLVM ld does not implement `-print-output-format`, and
upstream's Clang target table omits OpenBSD. `--gnu-tools` selects the separately
installed native gcc/binutils/bison/GNU sed/coreutils prerequisites through
private aliases. It compiles one native data object, records its exact GNU
objdump format and all tool hashes, and supplies that observed format to make.
It changes no upstream source or global executable alias. Keep this tools-only
attempt in a new work directory so earlier generated diagnostics remain intact.

The pure native timer control uses the reviewed nonblocking cancellation and
refcounted worker model from NetBSD 27341fd, with OpenBSD's monotonic pthread
condition API. Free never joins a callback, including one blocked on LKL CPU/IRQ
ownership. A separate quiescence drain is permitted only after CPU/thread/tick
shutdown and before freeing emulated memory. Native tests cover self-rearm,
self-free, a blocked callback with nonblocking free and delayed quiescence,
replacement/cancellation, large deadlines and 32 drained workers. This component
is not yet wired into an LKL build or a filesystem mount.
