# Native NetBSD LKL diagnostic build

This is an ordinary native host-process filesystem driver investigation. It
does not build a boot kernel, modify installed FUSE libraries, install a driver,
enable a Lurker filesystem, or change the qualified package. Its files belong
outside Lurker's private backend bundle.

The first phase captures unmodified current-upstream diagnostics. Run as an
ordinary NetBSD amd64 user on a work-specific filesystem with at least 6 GiB
initially free, using the native compiler and GNU make:

```sh
python3.14 scripts/build-netbsd-lkl.py --work-dir /path/to/lkl-work \
  --receipt /path/to/lkl-work/baseline.json --timeout 600
python3.14 scripts/test-build-netbsd-lkl.py
```

The source is pinned to `d0f76a77e8c2ca68bd7d96190b8da09040eab072` and archive
SHA-256 `ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168`.
Both cached archives and every upstream file are verified before resuming a
baseline. Existing receipts and logs are preserved. A deadline terminates the
owned make process group, including its compiler children. Build failures are
diagnostics, not qualified filesystem support.

After capturing the baseline, install the ordinary build dependencies `bison`,
`gmake`, `gsed`, and `coreutils`. The `--adapted` flag applies the hash-pinned
host patch and reuses the additive flush patch from
`native/macos/lkl-darwin-flush.patch` without changing existing disk-registration
semantics. It selects native ReFUSE API 38, GNU stat/sed aliases, BSD host mode,
and no MMU/KASAN or native networking transport. It refuses non-regular images
and requires `-f -s` if eventually qualified for execution. The patch currently
does **not** implement the credential/errno gates below: do not mount its output.

The private include overlay links to the actual `/usr/include/fuse*.h` headers.
It provides native byte-swap aliases for the compiler's `modpost` program and an
unmodified portable ELF file-format header from glibc 2.42, SHA-256
`fd6e7f3e3c74ee9d64b9f92ce3cc31b34413ca2622a167320ea053ce9a68110f`.
That header retains its LGPL 2.1-or-later notice and uses only `stdint.h`;
no glibc implementation or runtime library is used. It supplies the
cross-architecture relocation constants missing from NetBSD's native `elf.h`.
The exact native SDK/header hashes are recorded with each adapted build.
Upstream's header export uses private aliases to the interpreter running this
builder and to GNU sed/stat; their native paths and hashes are recorded. No
system executable or global alias is replaced. The overlay also maps the
Linux-style `endian.h` include to native `sys/endian.h`.
Native endian inlines are preloaded before LKL temporarily hides `__unused`
while parsing its UAPI; header discovery uses the same include-overlay flags
as host compilation. All compatibility files stay inside the build directory.
NetBSD's native `si_*` signal-field and `ipi_spec_dst` aliases are saved and
restored around that UAPI import as well. GNU sha1sum runs the upstream generated-header checks;
the final configuration and generated input hashes are retained in the receipt.

The mmap helper never passes `MAP_FIXED`. It accepts only the exact requested
address and releases only an alternate mapping returned by its own syscall.
Native tests cover collision preservation, failure cleanup, invalid alignment,
overflow and anonymous exact reuse. A shared-file fixed-base hint may be safely
rejected; these tests do not establish MMU availability.

Native ReFUSE API/flag adaptation, safe exact-address mmap hints, Linux-to-native
errno conversion, independently authenticated request credentials, explicit
block flush negotiation and native formatter compatibility remain required
gates before even file-backed runtime controls. No privileged product use is
approved by a successful compile. Preserve the installed providers and all
unrelated storage. Any usermount or device permission grant needs separate,
scoped native controls and restoration.

https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
https://man.netbsd.org/mmap.2
https://github.com/bminor/glibc/blob/glibc-2.42/elf/elf.h
