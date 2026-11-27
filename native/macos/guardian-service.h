/* SPDX-License-Identifier: MIT
 * User-only native guardian service and pre-thread worker spawn.
 * No embedded kernel, FUSE, mount, encryption, device or privilege operation.
 */
#ifndef LURKER_MACOS_GUARDIAN_SERVICE_H
#define LURKER_MACOS_GUARDIAN_SERVICE_H
#include "lkl-guardian.h"
#include <CommonCrypto/CommonDigest.h>
#include <limits.h>
#include <mach-o/dyld.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/file.h>
extern char **environ;

struct lgs_executable {
  int fd;
  struct stat identity;
  char path[PATH_MAX];
  uint8_t sha256[32];
};
/* Ordinary login-user candidate, never a privileged execution shortcut. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
static inline int lgs_self(struct lgs_executable *out) {
  char path[PATH_MAX];
  uint32_t length = sizeof(path);
  memset(out, 0, sizeof(*out));
  out->fd = -1;
  if (_NSGetExecutablePath(path, &length) || !realpath(path, out->path))
    return -1;
  out->fd = open(out->path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
  if (out->fd < 0)
    return -1;
  struct stat *st = &out->identity;
  if (fstat(out->fd, st) || !S_ISREG(st->st_mode) || st->st_uid != getuid() ||
      st->st_nlink != 1 || (st->st_mode & 0022) || st->st_size <= 0) {
    int error = errno ? errno : EPERM;
    close(out->fd);
    out->fd = -1;
    errno = error;
    return -1;
  }
  CC_SHA256_CTX hash;
  CC_SHA256_Init(&hash);
  unsigned char block[16384];
  off_t at = 0;
  for (;;) {
    ssize_t n = pread(out->fd, block, sizeof(block), at);
    if (n < 0 && errno == EINTR)
      continue;
    if (n < 0) {
      int error = errno;
      close(out->fd);
      out->fd = -1;
      errno = error;
      return -1;
    }
    if (!n)
      break;
    CC_SHA256_Update(&hash, block, (CC_LONG)n);
    at += n;
  }
  CC_SHA256_Final(out->sha256, &hash);
  struct stat after;
  if (fstat(out->fd, &after) || at != st->st_size ||
      after.st_dev != st->st_dev || after.st_ino != st->st_ino ||
      after.st_size != st->st_size ||
      after.st_mtimespec.tv_sec != st->st_mtimespec.tv_sec ||
      after.st_mtimespec.tv_nsec != st->st_mtimespec.tv_nsec) {
    close(out->fd);
    out->fd = -1;
    errno = ESTALE;
    return -1;
  }
  return 0;
}
#pragma clang diagnostic pop
static inline int lgs_same_executable(const struct lgs_executable *self) {
  struct stat held, named;
  if (fstat(self->fd, &held) || lstat(self->path, &named))
    return -1;
  if (!S_ISREG(named.st_mode) || held.st_dev != self->identity.st_dev ||
      held.st_ino != self->identity.st_ino || named.st_dev != held.st_dev ||
      named.st_ino != held.st_ino || named.st_size != held.st_size ||
      held.st_size != self->identity.st_size || held.st_uid != getuid() ||
      (held.st_mode & 0022) ||
      held.st_mtimespec.tv_sec != self->identity.st_mtimespec.tv_sec ||
      held.st_mtimespec.tv_nsec != self->identity.st_mtimespec.tv_nsec) {
    errno = ESTALE;
    return -1;
  }
  return 0;
}
/* Must run before LKL/FUSE or other worker threads. Duplicate inputs above the
 * fixed range first; CLOEXEC_DEFAULT closes every non-explicit inherited FD.
 * stdin is /dev/null; stdout/stderr deliberately retain guardian-owned logs.
 * Worker receives channel3 + source4 ONLY; guardian status is never an action.
 */
enum lgs_spawn_result {
  LGS_SPAWN_FAILED = -1,
  LGS_SPAWN_READY = 0,
  LGS_SPAWN_RETAINED = 1
};
static inline int lgs_spawn_worker(const struct lgs_executable *self,
                                   char *const argv[], int source_fd,
                                   pid_t *child, int *channel) {
  *child = 0;
  *channel = -1;
  if (!getuid() || geteuid() != getuid() || getegid() != getgid()) {
    errno = EPERM;
    return -1;
  }
  if (lgs_same_executable(self))
    return -1;
  struct stat source;
  int access = fcntl(source_fd, F_GETFL);
  if (fstat(source_fd, &source) || access < 0)
    return -1;
  if (!S_ISREG(source.st_mode) || source.st_uid != getuid() ||
      source.st_nlink != 1 || (source.st_mode & 0077) || source.st_size <= 0 ||
      (access & O_ACCMODE) != O_RDWR) {
    errno = EPERM;
    return -1;
  }
  if (flock(source_fd, LOCK_EX | LOCK_NB))
    return -1;
  int pair[2] = {-1, -1}, inputs[2] = {-1, -1};
  if (socketpair(AF_UNIX, SOCK_STREAM, 0, pair))
    return -1;
  for (int i = 0; i < 2; ++i)
    if (fcntl(pair[i], F_SETFD, FD_CLOEXEC))
      goto fail;
  inputs[0] = fcntl(pair[1], F_DUPFD_CLOEXEC, 16);
  inputs[1] = fcntl(source_fd, F_DUPFD_CLOEXEC, 16);
  if (inputs[0] < 0 || inputs[1] < 0)
    goto fail;
  posix_spawn_file_actions_t actions;
  posix_spawnattr_t attrs;
  int initialized_actions = 0, initialized_attrs = 0, error;
  error = posix_spawn_file_actions_init(&actions);
  if (error) {
    errno = error;
    goto fail;
  }
  initialized_actions = 1;
  error = posix_spawnattr_init(&attrs);
  if (error)
    goto spawn_fail;
  initialized_attrs = 1;
#define LGS_ACTION(call)                                                       \
  do {                                                                         \
    error = (call);                                                            \
    if (error)                                                                 \
      goto spawn_fail;                                                         \
  } while (0)
  LGS_ACTION(posix_spawnattr_setflags(&attrs, POSIX_SPAWN_CLOEXEC_DEFAULT));
  LGS_ACTION(
      posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0));
  LGS_ACTION(posix_spawn_file_actions_adddup2(&actions, 1, 1));
  LGS_ACTION(posix_spawn_file_actions_adddup2(&actions, 2, 2));
  LGS_ACTION(posix_spawn_file_actions_adddup2(&actions, inputs[0], 3));
  LGS_ACTION(posix_spawn_file_actions_adddup2(&actions, inputs[1], 4));
  error = posix_spawn(child, self->path, &actions, &attrs, argv, environ);
spawn_fail:
  if (initialized_attrs)
    posix_spawnattr_destroy(&attrs);
  if (initialized_actions)
    posix_spawn_file_actions_destroy(&actions);
  if (error) {
    errno = error;
    goto fail;
  }
#undef LGS_ACTION
  *channel = pair[0];
  int retained_error = 0;
  for (int i = 0; i < 2; ++i) {
    if (LG_CLOSE(inputs[i]) && !retained_error)
      retained_error = errno ? errno : EIO;
    inputs[i] = -1; /* actual close attempted; never retry uncertain close */
  }
  if (LG_CLOSE(pair[1]) && !retained_error)
    retained_error = errno ? errno : EIO;
  if (lgs_same_executable(self) && !retained_error)
    retained_error = errno ? errno : ESTALE;
  /* Any post-spawn error returns an owned-child/channel result, never the
   * no-child failure result. Caller MUST keep custody and reap this child. */
  if (retained_error) {
    errno = retained_error;
    return LGS_SPAWN_RETAINED;
  }
  return LGS_SPAWN_READY;
fail: {
  int saved = errno;
  for (int i = 0; i < 2; ++i) {
    if (inputs[i] >= 0)
      close(inputs[i]);
    if (pair[i] >= 0)
      close(pair[i]);
  }
  errno = saved;
  return -1;
}
}
struct lgs_service {
  struct lfgr_guardian *guardian;
  int listener_fd, progress_fd, directory_fd;
  char socket_name[sizeof(((struct sockaddr_un *)0)->sun_path)];
  struct stat socket_identity, directory_identity;
  uint8_t progress[512];
  size_t progress_bytes;
  uint64_t started, partial_started, close_started;
  unsigned startup_ms, frame_ms, completion_ms, rpc_ms;
  int channel_eof, status_error, last_error_persisted;
  unsigned rpc_replies, rpc_send_errors, rpc_refusals, worker_timeouts;
};
static inline int lgs_socket_identity(struct lgs_service *service) {
  struct stat dir, socket;
  if (fstat(service->directory_fd, &dir) ||
      fstatat(service->directory_fd, service->socket_name, &socket,
              AT_SYMLINK_NOFOLLOW))
    return -1;
  if (dir.st_dev != service->directory_identity.st_dev ||
      dir.st_ino != service->directory_identity.st_ino ||
      dir.st_uid != getuid() || (dir.st_mode & 0077) ||
      !S_ISSOCK(socket.st_mode) ||
      socket.st_dev != service->socket_identity.st_dev ||
      socket.st_ino != service->socket_identity.st_ino ||
      socket.st_uid != getuid() || (socket.st_mode & 0077) ||
      socket.st_nlink != 1) {
    errno = ESTALE;
    return -1;
  }
  return 0;
}
/* Caller owns the new0700 canonical directory and absent socket path. */
static inline int lgs_listen(struct lgs_service *service, const char *directory,
                             const char *name) {
  struct sockaddr_un address = {.sun_family = AF_UNIX};
  char path[PATH_MAX];
  service->listener_fd = service->directory_fd = -1;
  if (!name[0] || strchr(name, '/') ||
      strlen(name) >= sizeof(service->socket_name) ||
      snprintf(path, sizeof(path), "%s/%s", directory, name) >=
          (int)sizeof(address.sun_path)) {
    errno = ENAMETOOLONG;
    return -1;
  }
  service->directory_fd =
      open(directory, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (service->directory_fd < 0)
    return -1;
  if (fstat(service->directory_fd, &service->directory_identity) ||
      service->directory_identity.st_uid != getuid() ||
      (service->directory_identity.st_mode & 0077)) {
    errno = EPERM;
    return -1;
  }
  strcpy(service->socket_name, name);
  strcpy(address.sun_path, path);
  address.sun_len = sizeof(address);
  service->listener_fd = socket(AF_UNIX, SOCK_STREAM, 0);
  if (service->listener_fd < 0)
    return -1;
  if (fcntl(service->listener_fd, F_SETFD, FD_CLOEXEC) ||
      fcntl(service->listener_fd, F_SETFL, O_NONBLOCK))
    return -1;
  mode_t mask = umask(0077);
  int rc =
      bind(service->listener_fd, (struct sockaddr *)&address, sizeof(address));
  int saved = errno;
  umask(mask);
  if (rc) {
    errno = saved;
    return -1;
  }
  if (fstatat(service->directory_fd, name, &service->socket_identity,
              AT_SYMLINK_NOFOLLOW) ||
      fchmodat(service->directory_fd, name, 0600, 0) ||
      listen(service->listener_fd, 8))
    return -1;
  return lgs_socket_identity(service);
}
static inline int lgs_persist(struct lgs_service *service) {
  struct lfgr_guardian *g = service->guardian;
  if (g->ops->persist_status(g->native, g->status_fd, &g->current)) {
    lfgr_fail(g, errno ? errno : EIO, LFGR_S_STATUS);
    service->status_error = 1;
    return -1;
  }
  return 0;
}
static inline void lgs_worker_failure(struct lgs_service *service, int error) {
  lfgr_fail(service->guardian, error, LFGR_S_WORKER);
}
/* Nonblocking private worker framing. Consume queued frames before waitpid;
 * native socketpair creator credentials do not identify a spawned worker.
 * Exclusively inherited3/4 + verified child/session/component custody do. */
static inline void lgs_progress(struct lgs_service *service) {
  if (service->channel_eof)
    return;
  for (unsigned frames = 0; frames < 8;) {
    ssize_t n =
        recv(service->progress_fd, service->progress + service->progress_bytes,
             sizeof(service->progress) - service->progress_bytes, MSG_DONTWAIT);
    if (n < 0 && (errno == EINTR))
      continue;
    if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK))
      return;
    if (n < 0) {
      lgs_worker_failure(service, errno);
      service->channel_eof = 1;
      return;
    }
    if (!n) {
      service->channel_eof = 1;
      if (service->progress_bytes)
        lgs_worker_failure(service, EPROTO);
      lfgr_channel_closed(service->guardian);
      return;
    }
    if (!service->progress_bytes)
      service->partial_started = lg_milliseconds();
    service->progress_bytes += (size_t)n;
    if (service->progress_bytes == sizeof(service->progress)) {
      struct lfg_message m;
      if (lfg_decode(&m, service->progress, sizeof(service->progress)) !=
          LFG_OK)
        lgs_worker_failure(service, EPROTO);
      else if (!service->guardian->sticky_error) {
        if (lfgr_worker_record(service->guardian, &m) == 0)
          lgs_persist(service);
      }
      service->progress_bytes = 0;
      service->partial_started = 0;
      ++frames;
    }
  }
}
static inline void lgs_rpc(struct lgs_service *service) {
  int fd = accept(service->listener_fd, NULL, NULL);
  if (fd < 0)
    return;
  fcntl(fd, F_SETFD, FD_CLOEXEC);
  struct lg_process peer, again;
  struct lfg_message request, reply;
  uint64_t deadline = lg_milliseconds() + service->rpc_ms;
  if (lgs_socket_identity(service) || lg_peer(fd, &peer) ||
      lg_receive(fd, &request, deadline) ||
      lg_process_snapshot(peer.pid, &again) ||
      !lg_same_process(&again, peer.pid, peer.sec, peer.usec) ||
      again.uid != peer.uid ||
      lfgr_reply(service->guardian, &request, &reply)) {
    ++service->rpc_refusals;
    goto done;
  }
  if (request.kind == LFG_CLOSE && !service->guardian->wait_valid &&
      !service->guardian->sticky_error) {
    int absent = lfgr_check_absent(service->guardian);
    if (absent == 0 && !service->close_started)
      service->close_started = lg_milliseconds();
    if (lfgr_reply(service->guardian, &request, &reply))
      goto done;
  }
  /* Failed/lost replies never poison a completed live state. A new request can
   * retry using a fresh nonce while the same guardian remains available. */
  ++service->rpc_replies;
  if (lg_send(fd, &reply, deadline))
    ++service->rpc_send_errors;
done:
  close(fd);
}
/* One bounded native event step. Native owner decides service retirement;
 * errors retain custody and live negative replies, never force worker cleanup.
 * Public partial frames time out per connection; private partial/startup/close
 * completion timeouts permanently fail the session but keep it discoverable.
 */
static inline int lgs_step(struct lgs_service *service, unsigned wait_ms) {
  struct lfgr_guardian *g = service->guardian;
  struct pollfd p[2] = {{.fd = service->channel_eof ? -1 : service->progress_fd,
                         .events = POLLIN},
                        {.fd = service->listener_fd, .events = POLLIN}};
  int rc = poll(p, 2, (int)(wait_ms > 100 ? 100 : wait_ms));
  if (rc < 0 && errno != EINTR) {
    lfgr_fail(g, errno, LFGR_S_NATIVE);
    return -1;
  }
  lgs_progress(service);
  if (service->channel_eof || g->sticky_error)
    lfgr_reap(g, 1);
  uint64_t now = lg_milliseconds();
  if (!g->sticky_error) {
    if ((!g->ready && now - service->started >= service->startup_ms) ||
        (service->progress_bytes &&
         now - service->partial_started >= service->frame_ms) ||
        (service->close_started && !g->wait_valid &&
         now - service->close_started >= service->completion_ms)) {
      ++service->worker_timeouts;
      lgs_worker_failure(service, ETIMEDOUT);
    }
  }
  if (!g->sticky_error && !g->released && g->wait_valid && g->drained)
    lfgr_release(g);
  if (g->sticky_error && service->last_error_persisted != g->sticky_error) {
    service->last_error_persisted = g->sticky_error;
    lgs_persist(service);
  }
  if (p[1].revents & POLLIN)
    lgs_rpc(service);
  return g->sticky_error ? -1 : 0;
}
#endif
