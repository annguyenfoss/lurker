/* SPDX-License-Identifier: MIT
 * macOS host-only guardian primitives. No LKL calls, native mounts, elevation
 * or product admission. The worker's persisted record is not completion
 * authority; only this live parent, after actual wait and custody release, is.
 */
#ifndef LURKER_MACOS_LKL_GUARDIAN_H
#define LURKER_MACOS_LKL_GUARDIAN_H
#include "guardian-runtime.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <spawn.h>
#include <sys/proc.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#ifndef LG_TRACE
#define LG_TRACE(...) ((void)0)
#endif
#ifndef LG_FSYNC
#define LG_FSYNC fsync
#endif
#ifndef LG_FULLSYNC
#define LG_FULLSYNC(fd) fcntl((fd), F_FULLFSYNC)
#endif
#ifndef LG_CLOSE
#define LG_CLOSE close
#endif
#ifndef LG_PWRITE
#define LG_PWRITE pwrite
#endif
#ifndef LG_TRUNCATE
#define LG_TRUNCATE ftruncate
#endif
struct lg_process {
  pid_t pid;
  uid_t uid;
  uint64_t sec, usec;
};
/* Reject disappeared, exiting and zombie peers; compare the native process
 * snapshot around credential collection. No account-directory membership. */
static inline int lg_process_snapshot(pid_t pid, struct lg_process *out) {
  struct kinfo_proc a, b;
  size_t n = sizeof(a);
  int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, pid};
  if (pid <= 0) {
    errno = ESRCH;
    return -1;
  }
  memset(&a, 0, sizeof(a));
  memset(&b, 0, sizeof(b));
  if (sysctl(mib, 4, &a, &n, NULL, 0) || n != sizeof(a))
    return -1;
  n = sizeof(b);
  if (sysctl(mib, 4, &b, &n, NULL, 0) || n != sizeof(b))
    return -1;
  if (a.kp_proc.p_pid != pid || b.kp_proc.p_pid != pid ||
      (a.kp_proc.p_stat != SRUN && a.kp_proc.p_stat != SSLEEP &&
       a.kp_proc.p_stat != SSTOP) ||
      (b.kp_proc.p_stat != SRUN && b.kp_proc.p_stat != SSLEEP &&
       b.kp_proc.p_stat != SSTOP) ||
      (a.kp_proc.p_flag & P_WEXIT) || (b.kp_proc.p_flag & P_WEXIT) ||
      a.kp_proc.p_starttime.tv_sec != b.kp_proc.p_starttime.tv_sec ||
      a.kp_proc.p_starttime.tv_usec != b.kp_proc.p_starttime.tv_usec ||
      a.kp_eproc.e_ucred.cr_uid != b.kp_eproc.e_ucred.cr_uid ||
      a.kp_proc.p_starttime.tv_sec <= 0 || a.kp_proc.p_starttime.tv_usec < 0 ||
      a.kp_proc.p_starttime.tv_usec >= 1000000) {
    errno = ESRCH;
    return -1;
  }
  out->pid = pid;
  out->uid = a.kp_eproc.e_ucred.cr_uid;
  out->sec = (uint64_t)a.kp_proc.p_starttime.tv_sec;
  out->usec = (uint64_t)a.kp_proc.p_starttime.tv_usec;
  return 0;
}
/* Public accepted sockets only. A socketpair created before spawn has creator
 * credentials and is authenticated by inherited FD custody, not this API. */
static inline int lg_peer(int fd, struct lg_process *out) {
  pid_t pid = 0;
  uid_t uid;
  gid_t gid;
  socklen_t n = sizeof(pid);
  if (getpeereid(fd, &uid, &gid) ||
      getsockopt(fd, SOL_LOCAL, LOCAL_PEERPID, &pid, &n) || n != sizeof(pid))
    return -1;
  if (lg_process_snapshot(pid, out) || out->uid != uid || uid != getuid()) {
    errno = EPERM;
    return -1;
  }
  return 0;
}
static inline int lg_same_process(const struct lg_process *p, uint64_t pid,
                                  uint64_t sec, uint64_t usec) {
  return (uint64_t)p->pid == pid && p->sec == sec && p->usec == usec;
}
static inline int lg_regular_fd(int fd, uid_t uid, uint64_t dev, uint64_t ino,
                                uint64_t size, int exact_size) {
  struct stat st;
  if (fstat(fd, &st))
    return -1;
  if (!S_ISREG(st.st_mode) || st.st_uid != uid || st.st_nlink != 1 ||
      (st.st_mode & 0077) || (uint64_t)st.st_dev != dev ||
      (uint64_t)st.st_ino != ino || st.st_size < 0 ||
      (exact_size && (uint64_t)st.st_size != size)) {
    errno = ESTALE;
    return -1;
  }
  return 0;
}
static inline int lg_barrier(int fd) {
  if (LG_FSYNC(fd))
    return -1;
  return LG_FULLSYNC(fd);
}
/* A failure can leave a valid-looking frame in the file. Callers must retain
 * the in-memory failure and must never re-read the file as positive authority.
 */
static inline int lg_write_status_bytes(int fd, const struct lfg_message *m) {
  uint8_t frame[LFG_FRAME_SIZE];
  size_t done = 0;
  if (lfg_encode(frame, sizeof(frame), m) != LFG_OK) {
    errno = EPROTO;
    return -1;
  }
  while (done < sizeof(frame)) {
    ssize_t n = LG_PWRITE(fd, frame + done, sizeof(frame) - done, (off_t)done);
    if (n < 0 && errno == EINTR)
      continue;
    if (n <= 0) {
      if (!n)
        errno = EIO;
      return -1;
    }
    done += (size_t)n;
  }
  if (LG_TRUNCATE(fd, sizeof(frame)))
    return -1;
  return lg_barrier(fd);
}
/* Keep the guardian's status lock FD alive; check a separate writer reference
 * close after its final barrier. Same-OFD locks never substitute for waitpid.
 */
static inline int lg_write_status(int held_fd, const struct lfg_message *m) {
  int fd = fcntl(held_fd, F_DUPFD_CLOEXEC, 32);
  if (fd < 0)
    return -1;
  int rc = lg_write_status_bytes(fd, m), saved = errno;
  if (LG_CLOSE(fd)) {
    if (!rc)
      saved = errno;
    rc = -1;
  }
  errno = saved;
  return rc;
}
/* Mac binding only: shared state/reaping/release/reply lives in
 * guardian-runtime.h. Mount callbacks are mandatory and native-only; neither
 * callback may invoke emulated kernel calls from the supervisor. */
struct lg_native_binding {
  void *context;
  int (*validate_ready)(void *, const struct lfg_message *,
                        const struct lfg_message *);
  int (*native_absent)(void *, const struct lfg_message *);
};
static inline int lg_check_source(void *v, int fd,
                                  const struct lfg_message *m) {
  (void)v;
  if (m->source_kind != LFG_SOURCE_REGULAR || m->source_rdev ||
      m->source_logical_sector || m->source_physical_sector) {
    errno = EINVAL;
    return -1;
  }
  return lg_regular_fd(fd, (uid_t)m->uid, m->source_dev, m->source_ino,
                       m->source_size, 1);
}
static inline int lg_check_status(void *v, int fd,
                                  const struct lfg_message *m) {
  (void)v;
  return lg_regular_fd(fd, (uid_t)m->uid, m->status_dev, m->status_ino, 0, 0);
}
static inline int lg_validate_ready(void *v, const struct lfg_message *expected,
                                    const struct lfg_message *ready) {
  struct lg_native_binding *binding = v;
  return binding->validate_ready(binding->context, expected, ready);
}
static inline int lg_native_absent(void *v, const struct lfg_message *m) {
  struct lg_native_binding *binding = v;
  return binding->native_absent(binding->context, m);
}
static inline int lg_flush_source(void *v, int fd,
                                  const struct lfg_message *m) {
  if (lg_check_source(v, fd, m))
    return -1;
  return lg_barrier(fd);
}
static inline int lg_close_source(void *v, int *fd,
                                  const struct lfg_message *m) {
  if (lg_check_source(v, *fd, m))
    return -1;
  int closing = *fd;
  *fd = -1;
  return LG_CLOSE(closing);
}
static inline int lg_persist_status(void *v, int fd,
                                    const struct lfg_message *m) {
  if (lg_check_status(v, fd, m))
    return -1;
  if (lg_write_status(fd, m))
    return -1;
  return lg_check_status(v, fd, m);
}
static const struct lfgr_ops lg_runtime_ops = {
    lg_check_source, lg_check_status, lg_validate_ready, lg_native_absent,
    lg_flush_source, lg_close_source, lg_persist_status};
static inline int lg_runtime_init(struct lfgr_guardian *g,
                                  const struct lfg_message *m, pid_t child,
                                  int source_fd, int status_fd,
                                  struct lg_native_binding *binding) {
  if (!binding || !binding->validate_ready || !binding->native_absent) {
    errno = EINVAL;
    return -1;
  }
  return lfgr_init(g, m, child, source_fd, status_fd, &lg_runtime_ops, binding);
}
/* Native bounded public stream RPC. The request context comes from the owned
 * session; a status file can discover it but cannot make this exchange succeed
 * without the exact live guardian PID/birth and fresh nonce response. */
static inline uint64_t lg_milliseconds(void) {
  struct timespec t;
  if (clock_gettime(CLOCK_MONOTONIC, &t))
    return 0;
  return (uint64_t)t.tv_sec * 1000 + (uint64_t)t.tv_nsec / 1000000;
}
static inline int lg_socket_io(int fd, void *bytes, size_t length, int writing,
                               uint64_t deadline) {
  size_t at = 0;
  int one = 1;
  if (setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof(one)))
    return -1;
  while (at < length) {
    uint64_t now = lg_milliseconds();
    if (!now || now >= deadline) {
      errno = ETIMEDOUT;
      return -1;
    }
    uint64_t remaining = deadline - now;
    struct pollfd pollfd = {.fd = fd, .events = writing ? POLLOUT : POLLIN};
    int ready = poll(&pollfd, 1, (int)(remaining > 60000 ? 60000 : remaining));
    if (ready < 0 && errno == EINTR)
      continue;
    if (ready < 0)
      return -1;
    if (!ready)
      continue;
    if (pollfd.revents & (POLLERR | POLLNVAL)) {
      errno = EIO;
      return -1;
    }
    ssize_t n = writing
                    ? send(fd, (char *)bytes + at, length - at, MSG_DONTWAIT)
                    : recv(fd, (char *)bytes + at, length - at, MSG_DONTWAIT);
    if (n < 0 && (errno == EINTR || errno == EAGAIN))
      continue;
    if (n <= 0) {
      if (!n)
        errno = ECONNRESET;
      return -1;
    }
    at += (size_t)n;
  }
  return 0;
}
static inline int lg_receive(int fd, struct lfg_message *message,
                             uint64_t deadline) {
  uint8_t bytes[LFG_FRAME_SIZE];
  if (lg_socket_io(fd, bytes, sizeof(bytes), 0, deadline))
    return -1;
  if (lfg_decode(message, bytes, sizeof(bytes)) != LFG_OK) {
    errno = EPROTO;
    return -1;
  }
  return 0;
}
static inline int lg_send(int fd, const struct lfg_message *message,
                          uint64_t deadline) {
  uint8_t bytes[LFG_FRAME_SIZE];
  if (lfg_encode(bytes, sizeof(bytes), message) != LFG_OK) {
    errno = EPROTO;
    return -1;
  }
  return lg_socket_io(fd, bytes, sizeof(bytes), 1, deadline);
}
static inline int lg_exchange(int fd, const struct lfg_message *request,
                              struct lfg_message *reply, uint64_t deadline) {
  struct lg_process peer = {0};
  if (lg_peer(fd, &peer) || !lg_same_process(&peer, request->guardian_pid,
                                             request->guardian_birth_sec,
                                             request->guardian_birth_usec)) {
    LG_TRACE("peer mismatch got=%d expected=%llu sec=%llu/%llu usec=%llu/%llu "
             "errno=%d\n",
             peer.pid, (unsigned long long)request->guardian_pid,
             (unsigned long long)peer.sec,
             (unsigned long long)request->guardian_birth_sec,
             (unsigned long long)peer.usec,
             (unsigned long long)request->guardian_birth_usec, errno);
    errno = ESTALE;
    return -1;
  }
  if (lg_send(fd, request, deadline) || lg_receive(fd, reply, deadline))
    return -1;
  if (reply->kind != LFG_REPLY || !lfg_same_binding(request, reply) ||
      memcmp(request->native_id, reply->native_id, sizeof(reply->native_id)) ||
      memcmp(request->request_nonce, reply->request_nonce,
             sizeof(reply->request_nonce)) ||
      reply->sequence < request->sequence ||
      lg_process_snapshot((pid_t)request->guardian_pid, &peer) ||
      peer.uid != getuid() ||
      !lg_same_process(&peer, request->guardian_pid,
                       request->guardian_birth_sec,
                       request->guardian_birth_usec)) {
    LG_TRACE(
        "reply mismatch binding=%d native=%d nonce=%d sequence=%llu/%llu "
        "peer=%d\n",
        lfg_same_binding(request, reply),
        memcmp(request->native_id, reply->native_id, sizeof(reply->native_id)),
        memcmp(request->request_nonce, reply->request_nonce,
               sizeof(reply->request_nonce)),
        (unsigned long long)reply->sequence,
        (unsigned long long)request->sequence, peer.pid);
    errno = ESTALE;
    return -1;
  }
  return 0;
}
static inline int lg_call(const char *path, const struct lfg_message *request,
                          struct lfg_message *reply, uint64_t deadline) {
  struct sockaddr_un address = {.sun_family = AF_UNIX};
  struct stat before, after;
  size_t length = strlen(path);
  if (length >= sizeof(address.sun_path)) {
    errno = ENAMETOOLONG;
    return -1;
  }
  if (lstat(path, &before))
    return -1;
  if (!S_ISSOCK(before.st_mode) || before.st_uid != getuid() ||
      (before.st_mode & 0077) || before.st_nlink != 1) {
    errno = EPERM;
    return -1;
  }
  memcpy(address.sun_path, path, length + 1);
  address.sun_len = sizeof(address);
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (fd < 0)
    return -1;
  int rc = -1, saved;
  if (fcntl(fd, F_SETFD, FD_CLOEXEC) || fcntl(fd, F_SETFL, O_NONBLOCK))
    goto done;
  if (connect(fd, (struct sockaddr *)&address, sizeof(address))) {
    if (errno != EINPROGRESS)
      goto done;
    uint64_t now = lg_milliseconds();
    if (!now || now >= deadline) {
      errno = ETIMEDOUT;
      goto done;
    }
    struct pollfd p = {.fd = fd, .events = POLLOUT};
    int ready =
        poll(&p, 1, (int)((deadline - now) > 60000 ? 60000 : (deadline - now)));
    if (ready <= 0) {
      if (!ready)
        errno = ETIMEDOUT;
      goto done;
    }
    int error = 0;
    socklen_t n = sizeof(error);
    if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &error, &n) || n != sizeof(error))
      goto done;
    if (error) {
      errno = error;
      goto done;
    }
  }
  if (lstat(path, &after) || before.st_dev != after.st_dev ||
      before.st_ino != after.st_ino) {
    errno = ESTALE;
    goto done;
  }
  rc = lg_exchange(fd, request, reply, deadline);
done:
  saved = errno;
  if (close(fd)) {
    if (!rc)
      saved = errno;
    rc = -1;
  }
  errno = saved;
  return rc;
}
#endif
