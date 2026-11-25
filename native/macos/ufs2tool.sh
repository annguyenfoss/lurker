#!/bin/sh
# Private development launcher; the separately installed .NET 8 runtime is free.
set -eu
for prefix in /opt/homebrew /usr/local; do
    if [ -x "$prefix/opt/dotnet@8/bin/dotnet" ]; then
        export DOTNET_ROOT="$prefix/opt/dotnet@8/libexec"
        export DOTNET_CLI_TELEMETRY_OPTOUT=1
        exec "$prefix/opt/dotnet@8/bin/dotnet" "$(dirname "$0")/ufs2tool-runtime/UFS2Tool.dll" "$@"
    fi
done
echo 'UFS2 creation requires the free Homebrew dotnet@8 runtime.' >&2
exit 1
