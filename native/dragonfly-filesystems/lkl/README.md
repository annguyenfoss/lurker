# Native DragonFly LKL host driver

This source recipe describes a separately installed native filesystem driver.
It uses DragonFly threads, local device/file I/O and host FUSE. It is external to
Lurker's private package; it supplies no VM or network filesystem transport.

The baseline is the official DPorts `filesystems/lkl` recipe, its seven patches
and checksum-pinned upstream source listed in `recipe.json`. Compile as an
unprivileged user with one worker. Preserve baseline failures separately from
DragonFly compatibility patches. The first native baseline C failure was missing
foreign ELF relocation constants in the host `modpost` tool. Patch 0001 uses the
values already defined by that exact upstream source's x86, ARM and MIPS headers.

Runtime support is unqualified. Before admission, verify native ELF dependencies,
measured device geometry, flush/error forwarding, checked normal drain, source
and FUSE identity, actual created-file ownership, caller reads/writes and foreign
user denial. The Linux emulated filesystem credentials and errno values require
an explicit native boundary audit. Host process UID or a FUSE mount-owner gate
alone does not prove those properties. Existing positive Lurker matrices and
blocked CryFS integration are separate evidence.

UFS2 is not enabled by this recipe. Its conditional Linux write implementation
and a separate native UFS2 formatter need their own assessment and controls.
