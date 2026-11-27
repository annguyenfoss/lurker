# Linux host filesystem prerequisite

Unprivileged VeraCrypt uses two independent facilities. Lurker privately packages
its file-only VeraCrypt crypto engine and restricted native creator. A separately
installed `lurker-fs` runs filesystem code locally through LKL and host FUSE.
Neither LKL filesystem code nor the FUSE library belongs in a Lurker package.
There is no VM, network transport, loop device or device mapper in this route.

Build the host prerequisite as an ordinary user with native FUSE3 development
files and the build tools named by the recipe:

```sh
python native/linux-userspace/build-host-filesystem.py \
  --source-dir /path/to/host-filesystem-source-cache \
  --output-dir /path/to/new-host-filesystem-output --jobs 2
```

The recipe verifies the pinned source archive, stable patch and integration
patch through the shared source verifier, regenerates the reviewed configuration,
and emits the binary, notices and `host-filesystem-source.json`. It does not
install anything. The recipe keeps a 2 GiB free-space floor, stops its own
build process group if the floor is crossed, and preserves the source checkpoint.

A retained output can be verified without executing or rebuilding it:

```sh
python native/linux-userspace/build-host-filesystem.py --verify-output /path/to/host-output
```

This checks the recorded executable, configuration and source-input identities;
it does not relabel an earlier build as an execution of a newer recipe. Keep the
original executed recipe/log and any later verifier revision separately. The profile retains the existing filesystem selections; it
does not enable a new UFS1 driver. The LKL source and stable patch pins are in
`scripts/build-linux-userspace.py`. Upstream source:
https://github.com/lkl/linux

An administrator reviews the source/configuration, licenses, native FUSE ABI and
loader dependencies, then installs the executable with mode 0755 at
`/usr/local/libexec/lurker-fs` and retains its manifest/notices separately under
`/usr/local/share/lurker-fs`. All executable ancestors must be root-owned and
not writable by group or others. Install FUSE and `fusermount3` through the host
package manager. No private FUSE library is copied by this recipe. Drain active
sessions before replacing the host driver; never replace a running binary.

Lurker checks the host helper independently of its private crypto directory.
Other fixed locations are `/usr/local/bin/lurker-fs`, `/usr/libexec/lurker-fs`
and `/usr/bin/lurker-fs`. It requires `lurker-userspace-4`. Ordinary upstream
`lklfuse` is not a drop-in substitute: the integration includes source locking,
caller ownership, readiness/drain acknowledgements and virtual-block flush
forwarding to the actual host backing descriptor. Native qualification must
verify ordinary payload persistence, namespace behavior and host-FD fsync/error
propagation; an LKL-only fsync call is insufficient evidence.

Both processes run as the login user and refuse root. A missing or incompatible
prerequisite fails without elevation. `LURKER_USERSPACE_DIR` retains its legacy
full-SDK development layout; `LURKER_FILESYSTEM_HELPER` can select an explicit
host helper independently. These overrides are honored only in the unprivileged
login-user route, never elevated. They do not constitute an installed native
package or administrator-controlled host prerequisite.

Build application crypto separately:

```sh
python scripts/build-linux-userspace.py --crypto-only \
  --output-dir /path/to/new-private-crypto-output
```

The default is also crypto-only. `--only-creator` retains the narrower native
creator artifact. A `veracrypt-userspace` stage now requires the explicit
`private-crypto-external-filesystem` manifest contract and rejects old combined
SDKs containing `lurker-fs`. Keep historical SDK evidence under its original
identity; do not remove its driver and relabel its old envelope.
