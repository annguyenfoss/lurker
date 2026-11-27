/* SPDX-License-Identifier: MIT
 * macOS host-only guardian primitives. No LKL calls, native mounts, elevation
 * or product admission. The worker's persisted record is not completion
 * authority; only this live parent, after actual wait and custody release, is.
 */
#ifndef LURKER_MACOS_LKL_GUARDIAN_H
#define LURKER_MACOS_LKL_GUARDIAN_H
#include "guardian-protocol.h"
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
struct lg_guardian {
  struct lfg_message identity, current;
  pid_t child;
  int custody_fd, status_fd, wait_valid, wait_status;
  int sticky_error, error_stage, drained, ready, released;
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
static inline void lg_fail(struct lg_guardian *g, int e, int stage) {
  if (!g->sticky_error) {
    g->sticky_error = e > 0 ? e : EIO;
    g->error_stage = stage;
  }
  g->current.kind = LFG_REPLY;
  g->current.state = LFG_RETAINED;
  g->current.error = g->current.latched_error = (uint32_t)g->sticky_error;
  g->current.error_stage = (uint32_t)g->error_stage;
  ++g->current.sequence;
}
/* Owned-child waitpid is the only source for wait facts. No kill(pid,0),
 * process disappearance, foreign wait status or file content can substitute. */
static inline int lg_reap(struct lg_guardian *g, int nonblock) {
  int status;
  pid_t got;
  if (g->wait_valid)
    return 1;
  if (g->child <= 0 || (uint64_t)g->child != g->identity.worker_pid) {
    lg_fail(g, ECHILD, 1);
    errno = ECHILD;
    return -1;
  }
  do {
    got = waitpid(g->child, &status, nonblock ? WNOHANG : 0);
  } while (got < 0 && errno == EINTR);
  if (!got)
    return 0;
  if (got != g->child) {
    lg_fail(g, errno ? errno : ECHILD, 1);
    return -1;
  }
  g->wait_valid = 1;
  g->wait_status = status;
  if (WIFEXITED(status)) {
    g->current.wait_kind = LFG_WAIT_EXITED;
    g->current.wait_value = (uint32_t)WEXITSTATUS(status);
    g->current.flags |= LFG_F_WORKER_EXITED;
    if (!WEXITSTATUS(status))
      g->current.flags |= LFG_F_WORKER_EXIT_ZERO;
    else
      lg_fail(g, EIO, 2);
  } else {
    g->current.wait_kind = LFG_WAIT_SIGNALED;
    g->current.wait_value =
        WIFSIGNALED(status) ? (uint32_t)WTERMSIG(status) : 127;
    lg_fail(g, EINTR, 2);
  }
  return 1;
}
static inline int lg_worker_record(struct lg_guardian *g,
                                   const struct lfg_message *m) {
  if (lfg_validate(m) != LFG_OK || !lfg_same_binding(&g->identity, m) ||
      (g->ready &&
       memcmp(g->identity.native_id, m->native_id, sizeof(m->native_id)))) {
    lg_fail(g, ESTALE, 3);
    return -1;
  }
  if (m->kind == LFG_ERROR) {
    lg_fail(g, m->latched_error ? (int)m->latched_error : (int)m->error, 4);
    return -1;
  }
  if (m->kind == LFG_READY && !g->ready && !g->drained && !g->wait_valid) {
    memcpy(g->identity.native_id, m->native_id, sizeof(m->native_id));
    memcpy(g->current.native_id, m->native_id, sizeof(m->native_id));
    g->ready = 1;
    g->current.state = LFG_RUNNING;
    return 0;
  }
  if (m->kind == LFG_DRAINED && g->ready && !g->drained) {
    g->drained = 1;
    g->current.completed_checks = m->completed_checks;
    g->current.flags |= LFG_F_WORKER_DRAINED | LFG_F_NATIVE_CLOSED;
    if (!g->sticky_error)
      g->current.state = LFG_WORKER_DRAINED;
    return 0;
  }
  lg_fail(g, EPROTO, 3);
  return -1;
}
/* OS binding independently confirms captured native mount absence immediately
 * before this function. Passing 0/error is failure, never "not applicable". */
static inline int lg_release(struct lg_guardian *g, int native_closed) {
  if (g->released)
    return g->sticky_error ? -1 : 0;
  if (g->sticky_error)
    return -1;
  if (native_closed < 0) {
    lg_fail(g, -native_closed, 9);
    errno = -native_closed;
    return -1;
  }
  if (!g->wait_valid || !WIFEXITED(g->wait_status) ||
      WEXITSTATUS(g->wait_status) || !g->drained || !g->ready ||
      native_closed != 1 || g->current.completed_checks != LFG_C_ALL ||
      g->custody_fd < 0) {
    errno = EBUSY;
    return -1;
  }
  if ((g->identity.source_dev == g->identity.status_dev &&
       g->identity.source_ino == g->identity.status_ino) ||
      lg_regular_fd(g->status_fd, (uid_t)g->identity.uid,
                    g->identity.status_dev, g->identity.status_ino, 0, 0)) {
    lg_fail(g, ESTALE, 5);
    errno = ESTALE;
    return -1;
  }
  if (lg_regular_fd(g->custody_fd, (uid_t)g->identity.uid,
                    g->identity.source_dev, g->identity.source_ino,
                    g->identity.source_size, 1)) {
    lg_fail(g, errno, 5);
    return -1;
  }
  if (lg_barrier(g->custody_fd)) {
    lg_fail(g, errno, 6);
    return -1;
  }
  g->current.flags |= LFG_F_CUSTODY_FLUSHED;
  int fd = g->custody_fd;
  g->custody_fd = -1;
  if (LG_CLOSE(fd)) {
    lg_fail(g, errno, 7);
    return -1;
  }
  g->current.flags |= LFG_F_CUSTODY_CLOSED;
  g->current.kind = LFG_REPLY;
  g->current.state = LFG_RELEASED_CLEAN;
  g->current.flags |= LFG_F_STATUS_SYNCED;
  ++g->current.sequence;
  if (lg_write_status(g->status_fd, &g->current)) {
    lg_fail(g, errno, 8); /* leave file untouched: it is never authority */
    return -1;
  }
  g->released = 1;
  return 0;
}
/* Native authenticated RPC peer and socket inode/session-directory custody
 * are prerequisites. Fresh nonce is echoed; lost replies can retry. */
static inline int lg_reply(struct lg_guardian *g,
                           const struct lfg_message *request,
                           struct lfg_message *reply) {
  if (lfg_validate(request) != LFG_OK || !lfg_is_request(request->kind) ||
      !lfg_same_binding(&g->identity, request) ||
      memcmp(g->identity.native_id, request->native_id,
             sizeof(request->native_id))) {
    errno = ESTALE;
    return -1;
  }
  *reply = g->current;
  reply->kind = LFG_REPLY;
  memcpy(reply->request_nonce, request->request_nonce,
         sizeof(reply->request_nonce));
  if (g->sticky_error) {
    reply->state = LFG_RETAINED;
    reply->error = reply->latched_error = (uint32_t)g->sticky_error;
    reply->error_stage = (uint32_t)g->error_stage;
  }
  if (request->kind == LFG_RETIRE) {
    if (!g->released || g->sticky_error) {
      errno = EBUSY;
      return -1;
    }
    reply->state = LFG_RETIRED;
  }
  return lfg_validate(reply) == LFG_OK ? 0 : (errno = EPROTO, -1);
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
