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
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/proc.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <spawn.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
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

/* Production inheritance is channel 3 + source 4 ONLY. Status stays in the
 * guardian. All arguments, environment and executable trust/digest must be
 * validated by the native caller before this helper. No PATH lookup occurs.
 * With root privileges, also reject a writable/non-root executable ancestry.
 */
struct lfbg_spawn_inputs {
    const char *executable;
    char *const *argv;
    char *const *environment;
    int channel_fd, source_fd, status_fd, log_fd;
};
#ifndef LFBG_CLOSE_INTERNAL
#define LFBG_CLOSE_INTERNAL close
#endif
#ifndef LFBG_EXEC_STAT
#define LFBG_EXEC_STAT stat
#endif
enum lfbg_spawn_outcome {
    LFBG_SPAWN_FAILED = -1, LFBG_SPAWN_STARTED = 0, LFBG_SPAWN_RETAINED = 1
};
static inline int lfbg_connected_stream(int fd) {
    int type;
    socklen_t length = sizeof(type);
    struct sockaddr_un peer;
    socklen_t peer_length = sizeof(peer);
    if (getsockopt(fd, SOL_SOCKET, SO_TYPE, &type, &length) < 0 ||
        getpeername(fd, (struct sockaddr *)&peer, &peer_length) < 0) return -1;
    if (length != sizeof(type) || type != SOCK_STREAM ||
        peer.sun_family != AF_LOCAL) { errno = EINVAL; return -1; }
    return 0;
}
static inline int lfbg_private_regular(int fd, struct stat *out) {
    struct stat st;
    int flags = fcntl(fd, F_GETFL);
    if (flags < 0 || fstat(fd, &st) < 0) return -1;
    if (!S_ISREG(st.st_mode) || st.st_uid != geteuid() || st.st_nlink != 1 ||
        (st.st_mode & 0777) != 0600 || (flags & O_ACCMODE) == O_RDONLY) {
        errno = EPERM; return -1;
    }
    if (out) *out = st;
    return 0;
}
static inline int lfbg_root_executable(const char *path) {
    char canonical[PATH_MAX], parent[PATH_MAX];
    struct stat st;
    if (!realpath(path, canonical)) return -1;
    if (strcmp(path, canonical)) { errno = EPERM; return -1; }
    if (lstat(path, &st) < 0) return -1;
    if (!S_ISREG(st.st_mode) || st.st_uid || (st.st_mode & 0022) ||
        !(st.st_mode & 0111)) { errno = EPERM; return -1; }
    memcpy(parent, canonical, strlen(canonical) + 1);
    for (;;) {
        char *slash = strrchr(parent, '/');
        if (!slash) { errno = EINVAL; return -1; }
        if (slash == parent) parent[1] = 0; else *slash = 0;
        if (lstat(parent, &st) < 0) return -1;
        if (!S_ISDIR(st.st_mode) || st.st_uid || (st.st_mode & 0022)) {
            errno = EPERM; return -1;
        }
        if (slash == parent) return 0;
    }
}
/* Return -1 only without a published child, 0 when started, or 1 for an owned
 * child retained after setup/cleanup failure (with errno and *child valid).
 * The caller must retain that child and session/source custody. Never
 * retry an uncertain close or treat a cleanup error as "nothing spawned".
 */
static inline int lfbg_spawn_worker(const struct lfbg_spawn_inputs *in,
                                    pid_t *child) {
    struct kinfo_proc process;
    size_t process_length = sizeof(process);
    int mib[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()};
    struct stat source, status, log, executable_before, executable_after;
    int have_executable;
    int copies[4] = {-1, -1, -1, -1}, null_fd = -1;
    int actions_live = 0, attr_live = 0, error = 0, rc;
    posix_spawn_file_actions_t actions;
    posix_spawnattr_t attr;
    pid_t spawned = -1;
    sigset_t mask, defaults;
    if (!child) { errno = EINVAL; return -1; }
    *child = -1;
    if (!in || !in->executable || in->executable[0] != '/' || !in->argv ||
        !in->argv[0] || strcmp(in->argv[0], in->executable) ||
        !in->environment || in->status_fd < 0 ||
        in->status_fd == in->channel_fd || in->status_fd == in->source_fd ||
        in->status_fd == in->log_fd) { errno = EINVAL; return -1; }
    memset(&process, 0, sizeof(process));
    if (sysctl(mib, 4, &process, &process_length, NULL, 0) < 0) return -1;
    if (process_length != sizeof(process) || process.ki_numthreads != 1) {
        errno = EBUSY; return -1;
    }
    if (!geteuid() && lfbg_root_executable(in->executable) < 0) return -1;
    have_executable = !LFBG_EXEC_STAT(in->executable, &executable_before);
    if (lfbg_connected_stream(in->channel_fd) < 0 ||
        lfbg_private_regular(in->status_fd, &status) < 0 ||
        lfbg_private_regular(in->log_fd, &log) < 0 ||
        fstat(in->source_fd, &source) < 0) return -1;
    rc = fcntl(in->source_fd, F_GETFL);
    if (rc < 0) return -1;
    if ((!S_ISREG(source.st_mode) && !S_ISCHR(source.st_mode)) ||
        (rc & O_ACCMODE) != O_RDWR ||
        (source.st_dev == status.st_dev && source.st_ino == status.st_ino) ||
        (log.st_dev == status.st_dev && log.st_ino == status.st_ino) ||
        (log.st_dev == source.st_dev && log.st_ino == source.st_ino)) {
        errno = EINVAL; return -1;
    }
    rc = fcntl(in->status_fd, F_GETFD);
    if (rc < 0 || fcntl(in->status_fd, F_SETFD, rc | FD_CLOEXEC) < 0) return -1;
    copies[0] = fcntl(in->channel_fd, F_DUPFD_CLOEXEC, 16);
    if (copies[0] < 0) { error = errno; goto done; }
    copies[1] = fcntl(in->source_fd, F_DUPFD_CLOEXEC, 16);
    if (copies[1] < 0) { error = errno; goto done; }
    copies[2] = fcntl(in->log_fd, F_DUPFD_CLOEXEC, 16);
    if (copies[2] < 0) { error = errno; goto done; }
    null_fd = open("/dev/null", O_RDONLY | O_CLOEXEC);
    if (null_fd < 0) { error = errno; goto done; }
    copies[3] = fcntl(null_fd, F_DUPFD_CLOEXEC, 16);
    if (copies[3] < 0) { error = errno; goto done; }
    if ((rc = posix_spawn_file_actions_init(&actions))) { error = rc; goto done; }
    actions_live = 1;
    if ((rc = posix_spawnattr_init(&attr))) { error = rc; goto done; }
    attr_live = 1;
    sigemptyset(&mask); sigemptyset(&defaults);
    sigaddset(&defaults, SIGTERM); sigaddset(&defaults, SIGINT);
    sigaddset(&defaults, SIGHUP); sigaddset(&defaults, SIGPIPE);
    sigaddset(&defaults, SIGQUIT); sigaddset(&defaults, SIGCHLD);
#define LFBG_SPAWN_STEP(expr) do { if ((rc = (expr))) { error = rc; goto done; } } while (0)
    LFBG_SPAWN_STEP(posix_spawnattr_setflags(&attr,
        POSIX_SPAWN_SETPGROUP | POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF));
    LFBG_SPAWN_STEP(posix_spawnattr_setpgroup(&attr, 0));
    LFBG_SPAWN_STEP(posix_spawnattr_setsigmask(&attr, &mask));
    LFBG_SPAWN_STEP(posix_spawnattr_setsigdefault(&attr, &defaults));
    LFBG_SPAWN_STEP(posix_spawn_file_actions_adddup2(&actions, copies[3], 0));
    LFBG_SPAWN_STEP(posix_spawn_file_actions_adddup2(&actions, copies[2], 1));
    LFBG_SPAWN_STEP(posix_spawn_file_actions_adddup2(&actions, copies[2], 2));
    LFBG_SPAWN_STEP(posix_spawn_file_actions_adddup2(&actions, copies[0], 3));
    LFBG_SPAWN_STEP(posix_spawn_file_actions_adddup2(&actions, copies[1], 4));
    LFBG_SPAWN_STEP(posix_spawn_file_actions_addclosefrom_np(&actions, 5));
    LFBG_SPAWN_STEP(posix_spawn(&spawned, in->executable, &actions, &attr,
                               in->argv, in->environment));
    *child = spawned;
    if (!have_executable) error = ESTALE;
    else if (LFBG_EXEC_STAT(in->executable, &executable_after) < 0) error = errno;
    else if (executable_before.st_dev != executable_after.st_dev ||
        executable_before.st_ino != executable_after.st_ino ||
        executable_before.st_size != executable_after.st_size ||
        executable_before.st_mtim.tv_sec != executable_after.st_mtim.tv_sec ||
        executable_before.st_mtim.tv_nsec != executable_after.st_mtim.tv_nsec ||
        executable_before.st_ctim.tv_sec != executable_after.st_ctim.tv_sec ||
        executable_before.st_ctim.tv_nsec != executable_after.st_ctim.tv_nsec ||
        executable_before.st_uid != executable_after.st_uid ||
        executable_before.st_gid != executable_after.st_gid ||
        executable_before.st_mode != executable_after.st_mode) error = ESTALE;
#undef LFBG_SPAWN_STEP
done:
    if (attr_live && (rc = posix_spawnattr_destroy(&attr)) && !error) error = rc;
    if (actions_live && (rc = posix_spawn_file_actions_destroy(&actions)) && !error) error = rc;
    for (unsigned i = 0; i < 4; ++i)
        if (copies[i] >= 0 && LFBG_CLOSE_INTERNAL(copies[i]) < 0 && !error) error = errno;
    if (null_fd >= 0 && LFBG_CLOSE_INTERNAL(null_fd) < 0 && !error) error = errno;
    if (error) {
        errno = error;
        return *child > 0 ? LFBG_SPAWN_RETAINED : LFBG_SPAWN_FAILED;
    }
    return LFBG_SPAWN_STARTED;
}
/* First worker action, before any library/helper startup. A later exec must
 * never carry either the private channel or held provider into another image.
 */
static inline int lfbg_worker_fds_prepare(void) {
    if (lfbg_connected_stream(3) < 0) return -1;
    for (int fd = 3; fd <= 4; ++fd) {
        int flags = fcntl(fd, F_GETFD);
        if (flags < 0 || fcntl(fd, F_SETFD, flags | FD_CLOEXEC) < 0) return -1;
    }
    return 0;
}
#endif
