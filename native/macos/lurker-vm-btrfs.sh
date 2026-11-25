#!/bin/sh
# Embedded in lurker-core. Only public keys and nonsecret request geometry are
# substituted by the login-user supervisor. No default anylinuxfs NFS/control
# server is started by this shell-mode program.
set -eu
umask 077
TOKEN=__TOKEN__
CONTROL=__CONTROL__
MODE=__MODE__
READONLY=__READONLY__
SIZE=__SIZE__
OWNER_UID=__UID__
OWNER_GID=__GID__
PORT=__PORT__
PUBLIC_KEY=__PUBLIC_KEY__
R=/run/lurker-vm-$TOKEN
test "$(cat "$CONTROL/visible")" = "$TOKEN"
printf '__LURKER_VISIBLE__ %s\n' "$TOKEN"
mkdir -p "$R/export/data"
chmod 0700 "$R" "$R/export"
SSHD_PID=
MOUNTED=0
close_guest() {
    if test "$MOUNTED" = 1; then
        if ! sync; then
            printf '__LURKER_SYNC_FAILED__ %s\n' "$TOKEN"
            return 1
        fi
        if ! umount "$R/export/data"; then
            printf '__LURKER_BUSY__ %s\n' "$TOKEN"
            return 1
        fi
        MOUNTED=0
    fi
    if test -n "$SSHD_PID"; then
        kill -TERM "$SSHD_PID" 2>/dev/null || :
        wait "$SSHD_PID" || :
        SSHD_PID=
    fi
    printf '__LURKER_CLOSED__ %s\n' "$TOKEN"
}
on_exit() {
    status=$?
    trap - EXIT
    # Busy storage remains attached. Never turn a startup failure into a forced
    # guest unmount or a VM exit while the guest filesystem is still busy.
    while ! close_guest; do sleep 1; done
    exit "$status"
}
trap on_exit EXIT
# The supervisor journals the actual VM PID/start identity before releasing
# this gate, so neither format nor filesystem mount can outrun recovery state.
while ! test -f "$CONTROL/launch" || ! test "$(cat "$CONTROL/launch")" = "$TOKEN"; do
    if test -f "$CONTROL/stop" && test "$(cat "$CONTROL/stop")" = "$TOKEN"; then
        close_guest
        trap - EXIT
        exit 0
    fi
    sleep 0.2
done
for program in blockdev btrfs mount umount awk; do
    command -v "$program" >/dev/null || { echo "Missing guest tool: $program" >&2; exit 1; }
done
test -b /dev/vda
test "$(blockdev --getsize64 /dev/vda)" = "$SIZE"
# Only one raw filesystem is in scope, not a partition table or a multi-disk
# Btrfs set. Never scan/import other guest block devices to complete a set.
for partition in /sys/class/block/vda/*/partition; do
    test ! -f "$partition" || { echo 'Partitioned VM inputs are unsupported.' >&2; exit 1; }
done
if test "$MODE" = format; then
    test "$READONLY" = 0
    command -v mkfs.btrfs >/dev/null || { echo 'Missing guest mkfs.btrfs.' >&2; exit 1; }
    mkfs.btrfs -q -f /dev/vda
fi
if test "$READONLY" = 1; then
    blockdev --setro /dev/vda
    test "$(blockdev --getro /dev/vda)" = 1
fi
btrfs inspect-internal dump-super /dev/vda > "$R/superblock"
test "$(awk '$1=="num_devices" {print $2}' "$R/superblock")" = 1 || {
    echo 'Only single-device Btrfs filesystems are supported.' >&2
    exit 1
}
if test "$READONLY" = 1; then
    mount -t btrfs -o ro,nologreplay /dev/vda "$R/export/data"
else
    mount -t btrfs -o compress=zstd /dev/vda "$R/export/data"
fi
MOUNTED=1
if test "$MODE" = format; then
    chown "$OWNER_UID:$OWNER_GID" "$R/export/data"
    chmod 0700 "$R/export/data"
    close_guest
    trap - EXIT
    exit 0
fi
for program in ssh-keygen /usr/sbin/sshd; do
    command -v "$program" >/dev/null || { echo "Missing guest OpenSSH tool: $program" >&2; exit 1; }
done
mkdir -p /run/sshd
printf '%s\n' "$PUBLIC_KEY" > "$R/authorized_keys"
chmod 0600 "$R/authorized_keys"
# Unlock root's hash only in this VM's bind mount. Password authentication stays
# disabled; the installed host VirtioFS rootfs and other guests are unchanged.
awk -F: 'BEGIN {OFS=":"} $1=="root" {$2=""} {print}' /etc/shadow > "$R/shadow"
mount --bind "$R/shadow" /etc/shadow
ssh-keygen -q -t ed25519 -N '' -f "$R/hostkey"
cat > "$R/sshd_config" <<CONFIG
Port $PORT
ListenAddress 127.0.0.1
HostKey $R/hostkey
PidFile $R/sshd.pid
AuthorizedKeysFile $R/authorized_keys
AuthenticationMethods publickey
PubkeyAuthentication yes
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitEmptyPasswords no
PermitRootLogin prohibit-password
StrictModes no
AllowUsers root
AllowAgentForwarding no
AllowTcpForwarding no
X11Forwarding no
PermitTunnel no
UseDNS no
LogLevel ERROR
ForceCommand internal-sftp -d $R/export/data -u 0077
Subsystem sftp internal-sftp
CONFIG
/usr/sbin/sshd -t -f "$R/sshd_config"
printf '__LURKER_HOSTKEY__ %s %s\n' "$TOKEN" "$(cat "$R/hostkey.pub")"
/usr/sbin/sshd -D -e -f "$R/sshd_config" &
SSHD_PID=$!
while :; do
    if test -f "$CONTROL/stop" && test "$(cat "$CONTROL/stop")" = "$TOKEN"; then
        if close_guest; then
            trap - EXIT
            exit 0
        fi
        sleep 1
    fi
    if ! kill -0 "$SSHD_PID" 2>/dev/null; then
        echo 'Guest SFTP server exited.' >&2
        exit 1
    fi
    sleep 0.2
done
