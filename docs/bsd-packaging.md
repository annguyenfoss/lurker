# Native BSD packages

Build and stage the exact product commit on the target BSD using
`scripts/build-unix.py` or the verified remote runner. Then wrap that archive
on the same guest, as the build user. The wrapper verifies the archive digest,
source commit, native audit, packaged payload bytes, links and file ownership.
It preserves the original staged manifest inside the native package.

```sh
python scripts/build-bsd-package.py \
  --archive /absolute/stage/package.tar.gz \
  --sha256 FULL_STAGED_ARCHIVE_SHA256 --commit FULL_SOURCE_COMMIT \
  --version 0.1.0 --output /absolute/output/lurker-0.1.0.pkg
```

On NetBSD and OpenBSD, use `lurker-0.1.0.tgz` for the output name. Save the
JSON receipt printed by the wrapper. It identifies the source, archive and
package digests, package ABI, selected backends and desktop payload. An output
path already in use is refused.

| Target | Native tool | Install prefix |
| --- | --- | --- |
| FreeBSD | `pkg create` | `/usr/local` |
| DragonFly BSD | `pkg create` | `/usr/local` |
| NetBSD | `pkg_create` | `/usr/pkg` |
| OpenBSD | `pkg_create` | `/usr/local` |

Stage with the backends for the target. A CLI-only stage produces a CLI-only
package. Desktop packages need the native X11 libraries, xkbcommon and sudo.
Use the distribution's X11 sets on NetBSD and OpenBSD. The package installs
the private helper alongside the desktop binary. Administrators supply the
sudo policy and any native FUSE device access. FreeBSD PEFS requires the
`pefs-kmod` package matching the running kernel. The wrapper confirms that
the staged PEFS utility and matching module come from that package and adds
it as a dependency.

Use `pkg add` and `pkg delete` on FreeBSD and DragonFly. Use `pkg_add` and
`pkg_delete` on NetBSD and OpenBSD. OpenBSD's distribution policy requires a
trusted source or signed packages for normal distribution. Use `-Dunsigned`
only for a locally built artifact with verified provenance in a disposable
test. The wrapper creates artifacts and declares dependencies without
installing them or changing administrator policy.

During tests, keep the package database and install prefix both isolated.
OpenBSD `pkg_add -B` relocates payload paths and needs an explicit
`PKG_DBDIR` for database isolation. Seed the alternate root with the
distribution runtime dependencies before checking native install, and
verify the original database and install paths independently.
