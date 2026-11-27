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
#include <poll.h>
#include <stddef.h>
#include <time.h>
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

enum lfbg_peer_kind { LFBG_PUBLIC_LIVE = 1, LFBG_OWNED_PROGRESS = 2 };
struct lfbg_stream {
    int fd, authenticated, closed, error, discovering;
    enum lfbg_peer_kind kind;
    struct lfbg_process expected;
};
/* Set on BOTH endpoints before any peer can write, including before spawn.
 * Credentials are per write, not per partial read or logical 512-byte frame.
 */
static inline int lfbg_enable_credentials(int fd) {
    int one = 1;
    if (lfbg_connected_stream(fd) < 0) return -1;
    return setsockopt(fd, SOL_LOCAL, LOCAL_CREDS_PERSISTENT, &one, sizeof(one));
}
/* Apply before listen/connect. FreeBSD unp_copy_peercred inherits the listener
 * request on the nascent accepted socket before a client can write. */
static inline int lfbg_enable_listener_credentials(int fd) {
    int one=1,type;socklen_t length=sizeof(type);
    struct sockaddr_un local;socklen_t local_length=sizeof(local);
    if (getsockopt(fd,SOL_SOCKET,SO_TYPE,&type,&length)<0 ||
        getsockname(fd,(struct sockaddr *)&local,&local_length)<0) return -1;
    if (length!=sizeof(type) || type!=SOCK_STREAM || local.sun_family!=AF_LOCAL) {
        errno=EINVAL;return -1;
    }
    return setsockopt(fd,SOL_LOCAL,LOCAL_CREDS_PERSISTENT,&one,sizeof(one));
}
static inline int lfbg_peer_current(const struct lfbg_process *expected,
                                    enum lfbg_peer_kind kind) {
    if (!expected || !expected->pid || expected->pid > LFG_MAX_PID) {
        errno = EINVAL; return -1;
    }
    struct lfbg_process captured;
    struct kinfo_proc native;
    int mib[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, (pid_t)expected->pid};
    size_t length = sizeof(native);
    if (kind != LFBG_PUBLIC_LIVE && kind != LFBG_OWNED_PROGRESS) {
        errno = EINVAL; return -1;
    }
    if (lfbg_process_capture((pid_t)expected->pid, &captured) < 0) return -1;
    if (!lfbg_process_same(expected, &captured)) { errno = ESTALE; return -1; }
    memset(&native, 0, sizeof(native));
    if (sysctl(mib, 4, &native, &length, NULL, 0) < 0) return -1;
    if (length != sizeof(native) || native.ki_pid != (pid_t)expected->pid ||
        native.ki_start.tv_sec != (time_t)expected->birth_sec ||
        native.ki_start.tv_usec != (suseconds_t)expected->birth_usec ||
        native.ki_ruid != expected->real_uid || native.ki_uid != expected->effective_uid) {
        errno = ESTALE; return -1;
    }
    if (kind == LFBG_OWNED_PROGRESS) {
        /* The child's still-present zombie identity is useful ONLY before
         * owned waitpid. Drain channel/EOF before reaping; never infer exit. */
        if (native.ki_ppid != getpid()) { errno = EPERM; return -1; }
    } else if (native.ki_stat == SZOMB || native.ki_stat == SIDL ||
               (native.ki_flag & P_WEXIT)) { errno = ESRCH; return -1; }
    return 0;
}
static inline int lfbg_stream_init(struct lfbg_stream *out, int fd,
                                   const struct lfbg_process *peer,
                                   enum lfbg_peer_kind kind) {
    int enabled = 0;
    socklen_t length = sizeof(enabled);
    if (!out || !peer || (kind != LFBG_PUBLIC_LIVE && kind != LFBG_OWNED_PROGRESS)) {
        errno = EINVAL; return -1;
    }
    if (lfbg_connected_stream(fd) < 0 ||
        getsockopt(fd, SOL_LOCAL, LOCAL_CREDS_PERSISTENT, &enabled, &length) < 0)
        return -1;
    if (length != sizeof(enabled) || enabled != 1) { errno = EPERM; return -1; }
    if (lfbg_peer_current(peer, kind) < 0) return -1;
    *out = (struct lfbg_stream){.fd=fd, .kind=kind, .expected=*peer};
    return 0;
}
static inline int lfbg_stream_fail(struct lfbg_stream *stream, int error) {
    if (!stream->error) stream->error = error > 0 ? error : EIO;
    errno = stream->error;
    return -1;
}
static inline int lfbg_now_ms(int64_t *out) {
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) < 0) return -1;
    if (now.tv_sec < 0 || (uint64_t)now.tv_sec > (uint64_t)INT64_MAX / 1000) {
        errno = EOVERFLOW; return -1;
    }
    *out = (int64_t)now.tv_sec * 1000 + now.tv_nsec / 1000000;
    return 0;
}
static inline int lfbg_deadline(uint32_t milliseconds, int64_t *out) {
    int64_t now;
    if (!milliseconds || milliseconds > 30000) { errno = EINVAL; return -1; }
    if (lfbg_now_ms(&now) < 0) return -1;
    if (now > INT64_MAX - milliseconds) { errno = EOVERFLOW; return -1; }
    *out = now + milliseconds; return 0;
}
static inline int lfbg_stream_poll(struct lfbg_stream *s, short events,
                                   int64_t deadline) {
    for (;;) {
        int64_t now;
        if (s->error) { errno = s->error; return -1; }
        if (s->discovering) {
            if (s->kind!=LFBG_PUBLIC_LIVE || s->expected.pid || events!=POLLIN)
                return lfbg_stream_fail(s,EPERM);
        } else if (lfbg_peer_current(&s->expected, s->kind) < 0)
            return lfbg_stream_fail(s, errno);
        if (lfbg_now_ms(&now) < 0) return lfbg_stream_fail(s, errno);
        if (now >= deadline) return lfbg_stream_fail(s, ETIMEDOUT);
        int64_t remaining = deadline - now;
        if (remaining > 30000) return lfbg_stream_fail(s, EINVAL);
        struct pollfd fd = {.fd=s->fd, .events=events};
        int rc = poll(&fd, 1, (int)remaining);
        if (rc < 0 && errno == EINTR) continue;
        if (rc < 0) return lfbg_stream_fail(s, errno);
        if (!rc) return lfbg_stream_fail(s, ETIMEDOUT);
        if (fd.revents & POLLNVAL) return lfbg_stream_fail(s, EBADF);
        return 0;
    }
}
/* Reject all descriptors/unknown ancillary data. Close even rejected received
 * rights before returning, including a truncated control message. */
static inline int lfbg_credentials(struct lfbg_stream *s, struct msghdr *msg) {
    int bad = 0, seen = 0;
    struct lfbg_process candidate=s->expected;
    for (struct cmsghdr *c=CMSG_FIRSTHDR(msg); c; c=CMSG_NXTHDR(msg,c)) {
        size_t remaining = msg->msg_controllen -
            (size_t)((char *)c - (char *)msg->msg_control);
        if (c->cmsg_len < CMSG_LEN(0) || c->cmsg_len > remaining) {
            bad = EPROTO; break;
        }
        if (c->cmsg_level == SOL_SOCKET && c->cmsg_type == SCM_RIGHTS) {
            size_t bytes = c->cmsg_len - CMSG_LEN(0);
            for (size_t i=0; i + sizeof(int) <= bytes; i += sizeof(int)) {
                int fd; memcpy(&fd, (char *)CMSG_DATA(c)+i, sizeof(fd));
                if (fd >= 0) (void)close(fd);
            }
            bad = EPROTO; continue;
        }
        if (c->cmsg_level != SOL_SOCKET || c->cmsg_type != SCM_CREDS2 ||
            c->cmsg_len < CMSG_LEN(offsetof(struct sockcred2,sc_groups))) {
            bad = EPROTO; continue;
        }
        struct sockcred2 *cred = (struct sockcred2 *)CMSG_DATA(c);
        if (cred->sc_version != 0 || cred->sc_ngroups < 0 || cred->sc_ngroups > 1024 ||
            c->cmsg_len != CMSG_LEN(offsetof(struct sockcred2,sc_groups) +
                                    sizeof(gid_t)*(size_t)cred->sc_ngroups) ||
            cred->sc_pid<=0 || (uint64_t)cred->sc_pid>LFG_MAX_PID ||
            cred->sc_uid != s->expected.real_uid ||
            cred->sc_euid != s->expected.effective_uid) {bad = EPERM;continue;}
        if (s->discovering && !seen) {
            if (lfbg_process_capture(cred->sc_pid,&candidate)<0) {bad=errno;continue;}
            if (candidate.real_uid!=s->expected.real_uid ||
                candidate.effective_uid!=s->expected.effective_uid) {bad=EPERM;continue;}
        }
        if (cred->sc_pid!=(pid_t)candidate.pid) bad=EPERM;
        else seen = 1;
    }
    if (msg->msg_flags & (MSG_TRUNC | MSG_CTRUNC)) bad = EPROTO;
    if (!bad && !s->authenticated && !seen) bad = EPERM;
    if (bad) return lfbg_stream_fail(s,bad);
    if (lfbg_peer_current(&candidate,s->kind) < 0) return lfbg_stream_fail(s,errno);
    if (seen) {s->expected=candidate;s->authenticated=1;s->discovering=0;}
    return 0;
}
/* Return 1 only for a fully decoded frame, 0 for boundary EOF, -1 otherwise.
 * EOF is not child-exit or completion authority. Output stays unchanged on
 * EOF/error; partial/error streams are poisoned and cannot be reused.
 */
static inline int lfbg_receive_until(struct lfbg_stream *s,
                                     struct lfg_message *out, int64_t deadline) {
    uint8_t bytes[LFG_FRAME_SIZE]; size_t offset=0;
    if (!out || s->closed) { errno=EPROTO; return -1; }
    while (offset < sizeof(bytes)) {
        if (lfbg_stream_poll(s,POLLIN,deadline) < 0) return -1;
        union { struct cmsghdr align;
            unsigned char bytes[CMSG_SPACE(SOCKCRED2SIZE(1024))]; } control;
        struct iovec io={bytes+offset,sizeof(bytes)-offset};
        struct msghdr msg={.msg_iov=&io,.msg_iovlen=1,
            .msg_control=control.bytes,.msg_controllen=sizeof(control.bytes)};
        ssize_t n=recvmsg(s->fd,&msg,MSG_DONTWAIT|MSG_CMSG_CLOEXEC);
        if (n<0 && (errno==EINTR || errno==EAGAIN || errno==EWOULDBLOCK)) continue;
        if (n<0) return lfbg_stream_fail(s,errno);
        if (!n) {
            if ((msg.msg_controllen || (msg.msg_flags & (MSG_TRUNC | MSG_CTRUNC))) &&
                lfbg_credentials(s,&msg) < 0) return -1;
            if (lfbg_peer_current(&s->expected,s->kind) < 0)
                return lfbg_stream_fail(s,errno);
            s->closed=1;
            if (offset) return lfbg_stream_fail(s,EPROTO);
            return 0;
        }
        if (lfbg_credentials(s,&msg) < 0) return -1;
        offset+=(size_t)n;
    }
    struct lfg_message decoded;
    if (lfg_decode(&decoded,bytes,sizeof(bytes)) != LFG_OK)
        return lfbg_stream_fail(s,EPROTO);
    *out=decoded; return 1;
}
/* Server-side first frame, before its writer PID is known. The expected login
 * UID comes from held session policy, never the decoded request. Both the
 * connection's cached native UID and per-write real/effective UID must match.
 * Listener credentials must already have been enabled before publication.
 */
static inline int lfbg_public_receive_first(struct lfbg_stream *out,int fd,uid_t uid,
                                            struct lfg_message *message,int64_t deadline) {
    int enabled=0;socklen_t length=sizeof(enabled);uid_t peer_uid;gid_t peer_gid;
    if (!out || !message) {errno=EINVAL;return -1;}
    if (lfbg_connected_stream(fd)<0 || getpeereid(fd,&peer_uid,&peer_gid)<0 ||
        getsockopt(fd,SOL_LOCAL,LOCAL_CREDS_PERSISTENT,&enabled,&length)<0) return -1;
    if (peer_uid!=uid || length!=sizeof(enabled) || enabled!=1) {errno=EPERM;return -1;}
    *out=(struct lfbg_stream){.fd=fd,.kind=LFBG_PUBLIC_LIVE,.discovering=1,
        .expected={.real_uid=uid,.effective_uid=uid}};
    return lfbg_receive_until(out,message,deadline);
}
static inline int lfbg_send_until(struct lfbg_stream *s,
                                  const struct lfg_message *message,int64_t deadline) {
    uint8_t bytes[LFG_FRAME_SIZE]; size_t offset=0;
    if (s->closed || lfg_encode(bytes,sizeof(bytes),message) != LFG_OK) {
        errno=EPROTO; return -1;
    }
    while (offset<sizeof(bytes)) {
        if (lfbg_stream_poll(s,POLLOUT,deadline) < 0) return -1;
        ssize_t n=send(s->fd,bytes+offset,sizeof(bytes)-offset,MSG_DONTWAIT|MSG_NOSIGNAL);
        if (n<0 && (errno==EINTR || errno==EAGAIN || errno==EWOULDBLOCK)) continue;
        if (n<=0) return lfbg_stream_fail(s,n<0?errno:EIO);
        if (lfbg_peer_current(&s->expected,s->kind) < 0) return lfbg_stream_fail(s,errno);
        offset+=(size_t)n;
    }
    return 0;
}
/* Client creates a new nonce for every exchange; server replies may be replayed
 * only after correlation to that exchange and the entire immutable binding.
 */
static inline int lfbg_rpc_request(const struct lfg_message *identity,uint32_t kind,
                                   struct lfg_message *out) {
    if (!identity || !out) { errno=EINVAL;return -1; }
    struct lfg_message request=*identity;
    if (kind!=LFG_QUERY && kind!=LFG_CLOSE && kind!=LFG_RETIRE) { errno=EINVAL;return -1; }
    request.kind=kind; request.state=LFG_REQUEST; request.flags=0;
    request.completed_checks=request.error=request.latched_error=request.error_stage=0;
    request.wait_kind=request.wait_value=0; request.sequence=0;
    arc4random_buf(request.request_nonce,sizeof(request.request_nonce));
    if (!lfg_nonzero(request.request_nonce,sizeof(request.request_nonce)) ||
        lfg_validate(&request)!=LFG_OK) { errno=EPROTO;return -1; }
    *out=request;return 0;
}
static inline int lfbg_rpc_match(const struct lfg_message *identity,
                                 const struct lfg_message *request,
                                 const struct lfg_message *reply) {
    if (!identity || !request || !reply ||
        (request->kind!=LFG_QUERY && request->kind!=LFG_CLOSE && request->kind!=LFG_RETIRE) ||
        lfg_validate(request)!=LFG_OK || lfg_validate(reply)!=LFG_OK ||
        reply->kind!=LFG_REPLY || !lfg_same_binding(identity,request) ||
        !lfg_same_binding(identity,reply) ||
        memcmp(identity->native_id,request->native_id,sizeof(request->native_id)) ||
        memcmp(identity->native_id,reply->native_id,sizeof(reply->native_id)) ||
        memcmp(request->request_nonce,reply->request_nonce,sizeof(reply->request_nonce))) {
        errno=ESTALE;return -1;
    }
    if ((reply->state==LFG_RELEASED_CLEAN || reply->state==LFG_RETIRED) &&
        !lfg_nonzero((const uint8_t *)identity->native_id,sizeof(identity->native_id))) {
        errno=ESTALE;return -1;
    }
    return 0;
}

struct lfbg_status_identity { uint64_t device, inode; uint32_t uid; };
static inline int lfbg_status_capture(int fd,struct lfbg_status_identity *out) {
    struct stat st;
    if (!out) {errno=EINVAL;return -1;}
    if (lfbg_private_regular(fd,&st)<0) return -1;
    int flags=fcntl(fd,F_GETFL);
    if (flags<0) return -1;
    if ((flags&O_ACCMODE)!=O_RDWR || (st.st_mode&07777)!=0600 ||
        st.st_size<0 || st.st_size>LFG_FRAME_SIZE || !st.st_ino) {
        errno=EPERM;return -1;
    }
    *out=(struct lfbg_status_identity){st.st_dev,st.st_ino,st.st_uid};return 0;
}
static inline int lfbg_status_check(int fd,const struct lfbg_status_identity *expected) {
    struct lfbg_status_identity now;
    if (!expected) {errno=EINVAL;return -1;}
    if (lfbg_status_capture(fd,&now)<0) return -1;
    if (now.device!=expected->device || now.inode!=expected->inode || now.uid!=expected->uid) {
        errno=ESTALE;return -1;
    }
    return 0;
}
#ifndef LFBG_STATUS_PWRITE
#define LFBG_STATUS_PWRITE pwrite
#endif
#ifndef LFBG_STATUS_FSYNC
#define LFBG_STATUS_FSYNC fsync
#endif
#ifndef LFBG_STATUS_CLOSE
#define LFBG_STATUS_CLOSE close
#endif
/* Guardian only. Keep the original status/custody FD open even on error.
 * A duplicate writer shares its OFD/lock but is never child-exit proof. Return
 * success only after checked write/truncate/sync/identity and writer close.
 * Visible clean-looking bytes can survive an error: only the live guardian's
 * separately owned actual wait and complete state may authorize completion.
 */
static inline int lfbg_status_persist(int fd,const struct lfbg_status_identity *expected,
                                      const struct lfg_message *message) {
    uint8_t bytes[LFG_FRAME_SIZE];
    if (!message || lfg_encode(bytes,sizeof(bytes),message)!=LFG_OK) {errno=EPROTO;return -1;}
    if (lfbg_status_check(fd,expected)<0) return -1;
    int writer=fcntl(fd,F_DUPFD_CLOEXEC,16);
    if (writer<0) return -1;
    int error=0;size_t offset=0;
    if (lfbg_status_check(writer,expected)<0) error=errno;
    while (!error && offset<sizeof(bytes)) {
        ssize_t n=LFBG_STATUS_PWRITE(writer,bytes+offset,sizeof(bytes)-offset,(off_t)offset);
        if (n<0 && errno==EINTR) continue;
        if (n<=0) {error=n<0?errno:EIO;break;}
        if ((size_t)n>sizeof(bytes)-offset) {error=EIO;break;}
        offset+=(size_t)n;
    }
    if (!error && ftruncate(writer,sizeof(bytes))<0) error=errno;
    if (!error && LFBG_STATUS_FSYNC(writer)<0) error=errno;
    if (!error && lfbg_status_check(writer,expected)<0) error=errno;
    if (LFBG_STATUS_CLOSE(writer)<0 && !error) error=errno;
    /* Never retry close: its error can leave ownership ambiguous. */
    if (!error && lfbg_status_check(fd,expected)<0) error=errno;
    if (error) {errno=error;return -1;}
    return 0;
}

struct lfbg_endpoint {
    int directory_fd,listener_fd,ready;
    struct stat directory,socket,listener;
    char directory_path[PATH_MAX];
};
#define LFBG_SOCKET_NAME "guardian.sock"
#ifndef LFBG_ENDPOINT_CHMOD
#define LFBG_ENDPOINT_CHMOD fchmodat
#endif
static inline int lfbg_endpoint_check(const struct lfbg_endpoint *endpoint) {
    struct stat held,named,socket,listener;
    if (!endpoint || endpoint->directory_fd<0 || endpoint->listener_fd<0) {errno=EINVAL;return -1;}
    int directory_flags=fcntl(endpoint->directory_fd,F_GETFD),listener_flags=fcntl(endpoint->listener_fd,F_GETFD);
    if (directory_flags<0 || listener_flags<0) return -1;
    if (fstat(endpoint->directory_fd,&held)<0 || fstat(endpoint->listener_fd,&listener)<0 || lstat(endpoint->directory_path,&named)<0 ||
        fstatat(endpoint->directory_fd,LFBG_SOCKET_NAME,&socket,AT_SYMLINK_NOFOLLOW)<0) return -1;
    if (!S_ISDIR(held.st_mode) || !S_ISDIR(named.st_mode) ||
        held.st_dev!=endpoint->directory.st_dev || held.st_ino!=endpoint->directory.st_ino ||
        named.st_dev!=held.st_dev || named.st_ino!=held.st_ino ||
        held.st_uid!=geteuid() || named.st_uid!=geteuid() ||
        (held.st_mode&07777)!=0700 || (named.st_mode&07777)!=0700 ||
        !(directory_flags&FD_CLOEXEC) || !(listener_flags&FD_CLOEXEC) ||
        !S_ISSOCK(listener.st_mode) || listener.st_dev!=endpoint->listener.st_dev ||
        listener.st_ino!=endpoint->listener.st_ino ||
        !S_ISSOCK(socket.st_mode) || socket.st_dev!=endpoint->socket.st_dev ||
        socket.st_ino!=endpoint->socket.st_ino || socket.st_uid!=geteuid() ||
        socket.st_nlink!=1 || (socket.st_mode&07777)!=0600) {errno=ESTALE;return -1;}
    return 0;
}
/* Fresh existing canonical private directory only; never unlink an old name.
 * Return 0 ready, 1 retained after bind publication, -1 with no owned endpoint.
 * A retained result keeps the descriptors and requires explicit reconciliation.
 */
static inline int lfbg_endpoint_create(const char *directory,struct lfbg_endpoint *out) {
    struct lfbg_endpoint endpoint={.directory_fd=-1,.listener_fd=-1};
    int error=0;
    if (!out) {errno=EINVAL;return -1;}
    *out=endpoint;
    if (!directory || directory[0]!='/' || getuid()!=geteuid()) {errno=EPERM;return -1;}
    if (!realpath(directory,endpoint.directory_path)) return -1;
    if (strcmp(directory,endpoint.directory_path)) {errno=EPERM;return -1;}
    endpoint.directory_fd=open(directory,O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);
    if (endpoint.directory_fd<0) return -1;
    if (fstat(endpoint.directory_fd,&endpoint.directory)<0) {error=errno;goto no_endpoint;}
    if (!S_ISDIR(endpoint.directory.st_mode) || endpoint.directory.st_uid!=geteuid() ||
        (endpoint.directory.st_mode&07777)!=0700) {error=EPERM;goto no_endpoint;}
    endpoint.listener_fd=socket(AF_LOCAL,SOCK_STREAM|SOCK_CLOEXEC|SOCK_NONBLOCK,0);
    if (endpoint.listener_fd<0) {error=errno;goto no_endpoint;}
    if (fstat(endpoint.listener_fd,&endpoint.listener)<0) {error=errno;goto no_endpoint;}
    if (!S_ISSOCK(endpoint.listener.st_mode) || !endpoint.listener.st_ino) {error=EPERM;goto no_endpoint;}
    if (lfbg_enable_listener_credentials(endpoint.listener_fd)<0) {error=errno;goto no_endpoint;}
    struct sockaddr_un address={.sun_family=AF_LOCAL};
    memcpy(address.sun_path,LFBG_SOCKET_NAME,sizeof(LFBG_SOCKET_NAME));
    address.sun_len=(uint8_t)(offsetof(struct sockaddr_un,sun_path)+sizeof(LFBG_SOCKET_NAME));
    if (bindat(endpoint.directory_fd,endpoint.listener_fd,(struct sockaddr *)&address,address.sun_len)<0) {error=errno;goto no_endpoint;}
    *out=endpoint; /* From this point a named owned socket may exist. */
    if (fstatat(endpoint.directory_fd,LFBG_SOCKET_NAME,&endpoint.socket,AT_SYMLINK_NOFOLLOW)<0) {error=errno;goto retained;}
    *out=endpoint;
    if (LFBG_ENDPOINT_CHMOD(endpoint.directory_fd,LFBG_SOCKET_NAME,0600,AT_SYMLINK_NOFOLLOW)<0) {error=errno;goto retained;}
    if (lfbg_endpoint_check(out)<0 || listen(endpoint.listener_fd,4)<0) {error=errno;goto retained;}
    out->ready=1;return 0;
retained:
    errno=error?error:EIO;return 1;
no_endpoint:
    if (endpoint.listener_fd>=0) (void)close(endpoint.listener_fd);
    if (endpoint.directory_fd>=0) (void)close(endpoint.directory_fd);
    errno=error?error:EIO;return -1;
}
static inline int lfbg_endpoint_accept(const struct lfbg_endpoint *endpoint) {
    if (!endpoint || !endpoint->ready) {errno=EINVAL;return -1;}
    if (lfbg_endpoint_check(endpoint)<0) return -1;
    int fd=accept4(endpoint->listener_fd,NULL,NULL,SOCK_CLOEXEC|SOCK_NONBLOCK);
    if (fd<0) return -1;
    if (lfbg_endpoint_check(endpoint)<0) {int error=errno;(void)close(fd);errno=error;return -1;}
    return fd;
}
/* Only for an already quiescent service after its separate protocol authority.
 * This helper grants no worker/storage completion. Never remove a replacement.
 */
static inline int lfbg_endpoint_close(struct lfbg_endpoint *endpoint) {
    if (lfbg_endpoint_check(endpoint)<0) return -1;
    if (unlinkat(endpoint->directory_fd,LFBG_SOCKET_NAME,0)<0) return -1;
    int fd=endpoint->listener_fd;endpoint->listener_fd=-1;endpoint->ready=0;
    if (close(fd)<0) return -1;
    fd=endpoint->directory_fd;endpoint->directory_fd=-1;
    return close(fd);
}
#endif
