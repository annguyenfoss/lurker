/* SPDX-License-Identifier: MIT
 * Native FreeBSD identity/provider helpers for the external host guardian.
 * No mount, unmount, signal, protocol authorization or encryption operation.
 */
#ifndef LURKER_FREEBSD_LKL_GUARDIAN_PLATFORM_H
#define LURKER_FREEBSD_LKL_GUARDIAN_PLATFORM_H
#ifndef __FreeBSD__
#error This platform binding requires genuine FreeBSD
#endif
#ifndef LFG_VERSION
#error Include the reviewed common guardian-protocol.h first
#endif
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#include <sys/user.h>
#include <sys/time.h>
#include <sys/disk.h>
#include <sys/ioctl.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

struct lfbg_process {
    uint64_t pid, birth_sec, birth_usec, boot_sec, boot_usec;
    uint32_t real_uid, effective_uid;
};
struct lfbg_source {
    uint64_t device, inode, rdev, bytes;
    uint32_t kind, logical_sector, physical_sector;
};

static inline int lfbg_process_capture(pid_t pid, struct lfbg_process *out) {
    struct kinfo_proc native;
    struct timeval boot;
    struct lfbg_process captured = {0};
    int mib[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, pid};
    size_t length = sizeof(native);
    if (!out || pid <= 0 || (uint64_t)pid > LFG_MAX_PID) {
        errno = EINVAL; return -1;
    }
    memset(&native, 0, sizeof(native));
    if (sysctl(mib, 4, &native, &length, NULL, 0) < 0) return -1;
    if (length != sizeof(native) || native.ki_pid != pid ||
        native.ki_start.tv_sec <= 0 || native.ki_start.tv_usec < 0 ||
        native.ki_start.tv_usec >= 1000000) {
        errno = ESRCH; return -1;
    }
    length = sizeof(boot);
    if (sysctlbyname("kern.boottime", &boot, &length, NULL, 0) < 0) return -1;
    if (length != sizeof(boot) || boot.tv_sec <= 0 || boot.tv_usec < 0 ||
        boot.tv_usec >= 1000000) {
        errno = EINVAL; return -1;
    }
    captured.pid = (uint64_t)pid;
    captured.birth_sec = (uint64_t)native.ki_start.tv_sec;
    captured.birth_usec = (uint64_t)native.ki_start.tv_usec;
    captured.boot_sec = (uint64_t)boot.tv_sec;
    captured.boot_usec = (uint64_t)boot.tv_usec;
    captured.real_uid = native.ki_ruid;
    captured.effective_uid = native.ki_uid;
    *out = captured;
    return 0;
}
static inline int lfbg_process_same(const struct lfbg_process *a,
                                     const struct lfbg_process *b) {
    return a->pid == b->pid && a->birth_sec == b->birth_sec &&
        a->birth_usec == b->birth_usec && a->boot_sec == b->boot_sec &&
        a->boot_usec == b->boot_usec && a->real_uid == b->real_uid &&
        a->effective_uid == b->effective_uid;
}
static inline void lfbg_process_boot_bytes(const struct lfbg_process *process,
                                           uint8_t out[16]) {
    lfg_put64(out, process->boot_sec);
    lfg_put64(out + 8, process->boot_usec);
}

static inline int lfbg_source_capture(int fd, struct lfbg_source *out) {
    struct stat metadata;
    struct lfbg_source captured = {0};
    if (!out) { errno = EINVAL; return -1; }
    if (fstat(fd, &metadata) < 0) return -1;
    captured.device = metadata.st_dev;
    captured.inode = metadata.st_ino;
    if (S_ISREG(metadata.st_mode)) {
        if (metadata.st_size <= 0) { errno = EINVAL; return -1; }
        captured.kind = LFG_SOURCE_REGULAR;
        captured.bytes = (uint64_t)metadata.st_size;
    } else if (S_ISCHR(metadata.st_mode)) {
        off_t media;
        unsigned logical;
        if (ioctl(fd, DIOCGMEDIASIZE, &media) < 0 ||
            ioctl(fd, DIOCGSECTORSIZE, &logical) < 0) return -1;
        if (media <= 0 || logical < 512 || logical > 65536 ||
            (logical & (logical - 1)) || (uint64_t)media % logical ||
            metadata.st_rdev == 0) { errno = EINVAL; return -1; }
        captured.kind = LFG_SOURCE_CHAR_PROVIDER;
        captured.rdev = metadata.st_rdev;
        captured.bytes = (uint64_t)media;
        captured.logical_sector = logical;
        /* Explicit FreeBSD policy: this interface provides logical size only.
         * Physical sector is UNKNOWN. DIOCGSTRIPESIZE is not a substitute. */
        captured.physical_sector = 0;
    } else { errno = EINVAL; return -1; }
    if (!captured.inode || captured.bytes > LFG_MAX_OFFSET) {
        errno = EINVAL; return -1;
    }
    *out = captured;
    return 0;
}
static inline int lfbg_source_same(const struct lfbg_source *a,
                                    const struct lfbg_source *b) {
    return a->device == b->device && a->inode == b->inode &&
        a->rdev == b->rdev && a->bytes == b->bytes && a->kind == b->kind &&
        a->logical_sector == b->logical_sector &&
        a->physical_sector == b->physical_sector;
}
static inline int lfbg_source_flush(int fd, const struct lfbg_source *expected) {
    struct lfbg_source current;
    if (lfbg_source_capture(fd, &current) < 0) return -1;
    if (!lfbg_source_same(expected, &current)) { errno = ESTALE; return -1; }
    return current.kind == LFG_SOURCE_REGULAR ? fsync(fd) : ioctl(fd, DIOCGFLUSH);
}
/* Never retry an ambiguous close or reopen/adopt its old pathname. The caller
 * must latch any failure, preserve logical session custody and deny success. */
static inline int lfbg_source_close(int *fd) {
    int owned, result;
    if (!fd || *fd < 0) { errno = EBADF; return -1; }
    owned = *fd;
    *fd = -1;
    result = close(owned);
    return result;
}
#endif
