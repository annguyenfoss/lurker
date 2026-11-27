#!/usr/bin/env bash
# Every operation uses the calling user and newly allocated disposable files.
set -euo pipefail
if [[ $EUID == 0 ]]; then
    echo 'Run the userspace suite as your login user, without sudo.' >&2
    exit 1
fi
lurker_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$lurker_root"
export LURKER_USERSPACE_DIR=${LURKER_USERSPACE_DIR:-$lurker_root/target/release/userspace}
# An explicit legacy full SDK may still provide its own driver for development.
if [[ -z ${LURKER_FILESYSTEM_HELPER:-} && -x $LURKER_USERSPACE_DIR/lurker-fs ]]; then
    export LURKER_FILESYSTEM_HELPER=$LURKER_USERSPACE_DIR/lurker-fs
else
    export LURKER_FILESYSTEM_HELPER=${LURKER_FILESYSTEM_HELPER:-/usr/local/libexec/lurker-fs}
fi
if [[ ! -x $LURKER_USERSPACE_DIR/lurker-veracrypt || ! -x $LURKER_FILESYSTEM_HELPER ]]; then
    echo 'Build private crypto with --crypto-only and install the host filesystem prerequisite; see native/linux-userspace/HOST_FILESYSTEM.md.' >&2
    exit 1
fi

cargo test --locked --offline -p lurker-core --test userspace_workflows \
    -- --ignored --nocapture --test-threads=1 "$@"
