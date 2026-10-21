#!/usr/bin/env bash
# Compile and run the exact current integration-test artifact; never reuse a
# filename from an earlier Cargo build. All volumes are newly allocated fixtures.
set -euo pipefail
lurker_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$lurker_root"
lurker_test_binary=$(cargo test --locked --offline -p lurker-core \
    --test linux_workflows --no-run --message-format=json \
    --target-dir target/linux-integration | python -c '
import json, sys
artifacts = [item["executable"] for line in sys.stdin
             if (item := json.loads(line)).get("reason") == "compiler-artifact"
             and item.get("target", {}).get("name") == "linux_workflows"
             and item.get("executable")]
assert len(artifacts) == 1, artifacts
print(artifacts[0])
')
if [[ $EUID == 0 ]]; then
    "$lurker_test_binary" --ignored --nocapture --test-threads=1 "$@"
else
    lurker_test_env=()
    if [[ -n ${LURKER_TEST_CASE:-} ]]; then
        lurker_test_env=("LURKER_TEST_CASE=$LURKER_TEST_CASE")
    fi
    sudo -n env "${lurker_test_env[@]}" "$lurker_test_binary" --ignored --nocapture --test-threads=1 "$@"
fi
