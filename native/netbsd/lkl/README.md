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

Native ReFUSE API/flag adaptation, safe exact-address mmap hints, Linux-to-native
errno conversion, independently authenticated request credentials, explicit
block flush negotiation and native formatter compatibility remain required
gates before even file-backed runtime controls. No privileged product use is
approved by a successful compile. Preserve the installed providers and all
unrelated storage. Any usermount or device permission grant needs separate,
scoped native controls and restoration.

https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
https://man.netbsd.org/mmap.2
