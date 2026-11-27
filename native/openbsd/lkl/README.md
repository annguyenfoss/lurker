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

The native memory control checks pthread stack-top conversion against a real
local variable, refuses destructive fixed mappings, preserves an occupied hint,
and verifies that only its own alternate mapping is released. A read-only
mquery checks that cleanup; the mapping implementation never uses query-then-map
or MAP_FIXED. Failed mmap, unaligned/overflow requests, anonymous reuse and an
actual shared-file mapping are covered. This primitive is not yet wired into
LKL, and MMU remains outside the initial boot profile.

`--adapted --gnu-tools` applies the reviewed OpenBSD host-only patch and copies
its separately hash-pinned timer/memory components into a fresh uncompiled
source tree. Existing configured trees are refused before the additive host_ops
field is introduced. Exported kernel/host headers and all objects are built
together. The adapter preserves native signal macros around Linux UAPI imports,
requires no MMU/KASAN, and disables native network backends/Linux hijack targets.
The inherited emulated CONFIG_NET=y remains recorded; it supplies no host or
network-filesystem transport.

The host compiler overlay consists only of a native endian alias header and
unchanged glibc2.42 ELF file-format declarations (SHA-256
`fd6e7f3e3c74ee9d64b9f92ce3cc31b34413ca2622a167320ea053ce9a68110f`).
The latter retains its LGPL2.1-or-later notice and imports only stdint.h;
no glibc runtime or FUSE library is copied. The first two unmodified diagnostics
remain separate: base linker/Clang configuration, then missing byteswap.h in
actual native GCC16 modpost compilation. No filesystem-driver runtime credit
follows from these diagnostics or the native primitive checks.

GNU-tool builds require `--bc /absolute/native/gnu-bc`. OpenBSD base bc is not
compatible with Linux timeconst.bc: removing its unsupported -q still emitted
HZ0 and a parser error despite exit0. The qualified build-tool candidate is
native GNU bc1.08.2 from https://ftp.gnu.org/gnu/bc/bc-1.08.2.tar.gz,
SHA-256 `ae470fec429775653e042015edc928d07c8c3b2fc59765172a330d3d87785f86`,
verified against the official NetBSD pkgsrc SHA-512/BLAKE2s/size record. Build it
as an ordinary user; keep it private to the build workspace. The producer only
adds a private alias and records its path/hash/version; it does not replace base
bc or elevate this tool. Native GNU bc generated the actual HZ100 constants
successfully; the incompatible-base attempt remains preserved.

`host-contract.c` validates the actual generated Linux stat, signal, packet and
directory-entry layouts while proving native signal aliases are restored.
Compile with `LURKER_LKL_ABI_ONLY` for that check alone. Its full form links the
exact native library and exercises only RAM boot, getpid, a measured nanosleep,
halt and the explicit post-CPU, pre-free-memory quiescence callback. It registers
no host disk, links no FUSE library and must run as the ordinary qualification
user. Host mounts must remain unchanged. This is a library/ABI/timer proof;
filesystem callbacks, request credentials, barriers, normal-close handling and
native formatters remain unqualified.
