# DragonFly gocryptfs userspace ABI batch

This is an unprivileged compile/ABI batch, not backend admission. It uses pinned
upstream gocryptfs and Go-FUSE with native DragonFly adapters. It does not use
LKL, alter a kernel or change crypto profiles. No mount, broker or live FUSE-FD
adoption is authorized by these checks. Never run `go test ./...` here: upstream
integration tests can mount filesystems. Select only reviewed pure tests.

`freeze.py` requires native DragonFly UID 1300, the existing Go 1.26.11 toolchain,
verified source/module caches and a new private workspace. It retains the prior
pristine compiler failure and original source manifests, then compiles/runs
`abi-probe.c`. Work/cache space may use the already identified work disk solely
as storage; no archived LKL program or library is used. Initial measured new-data
budget is 256 MiB, with 512 MiB work and 768 MiB root reserves. The freeze
records free capacity for every output/cache filesystem, directs compiler
temporary files into the new workspace, and disables Go downloads and telemetry.
It fsyncs source copies, header snapshots, manifests, the probe and their
directories before returning. Collect the complete evidence directory off-host
immediately; a missing earlier receipt is never reconstructed as a prior pass.

Native installed public headers supply dirent/stat/statfs, flags and syscall
identities. The FUSE wire header is the published unmodified stock source
`e17b9176356e3afb3427d71c21108a8a849cd4d5/sys/vfs/fuse/fuse_abi.h`, not an installed
header. Both categories have exact native SHA-256 receipts. This distinction
matters: DragonFly wire INIT is 64 bytes, while its native directory entries
have no FreeBSD offset/reclen fields. There is no copied FreeBSD syscall number
or structure cast. Unsupported optional operations must remain explicit.

Completion of the ABI batch is separate from FilesystemSupport. Compilation or
pure marshalling tests do not establish caller I/O, FUSE readiness, close safety,
filesystem backing support or product capability. Shared core/API/stager changes
and actual transport qualification require a later coordinated review.
