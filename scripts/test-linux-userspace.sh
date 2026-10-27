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
if [[ ! -x $LURKER_USERSPACE_DIR/lurker-veracrypt || ! -x $LURKER_USERSPACE_DIR/lurker-fs ]]; then
    echo 'Build the helpers first: python scripts/build-linux-userspace.py' >&2
    exit 1
fi
cargo test --locked --offline -p lurker-core --test userspace_workflows \
    -- --ignored --nocapture --test-threads=1 "$@"
