/* SPDX-License-Identifier: MIT
 * Native POSIX controls. No embedded kernel, FUSE, mount or device operations.
 * Native mount outcomes and captured births are explicit control inputs only.
 */
#include "../guardian-runtime.h"
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/file.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <unistd.h>
extern char **environ;
static unsigned checks;
#define CHECK(x)                                                               \
  do {                                                                         \
    ++checks;                                                                  \
    if (!(x)) {                                                                \
      fprintf(stderr, "line%u check%u %s errno=%d\n", __LINE__, checks, #x,    \
              errno);                                                          \
      exit(1);                                                                 \
    }                                                                          \
  } while (0)
struct fixture {
  const char *mode;
  struct lfgr_guardian *guardian;
  dev_t status_dev, source_dev;
  ino_t status_ino, source_ino;
  int source_sync_fail, status_sync_fail, status_write_fail, source_close_fail,
      status_close_fail;
  int absent_calls, flush_calls, close_calls, persist_calls, ready_calls;
};
static int barrier(int fd) {
  if (fsync(fd))
    return -1;
#ifdef __APPLE__
  return fcntl(fd, F_FULLFSYNC);
#else
  return 0;
#endif
}
static int inspect(int fd, uid_t uid, uint64_t dev, uint64_t ino,
                   uint64_t bytes, int exact) {
  struct stat s;
  if (fstat(fd, &s))
    return -1;
  if (!S_ISREG(s.st_mode) || s.st_uid != uid || s.st_nlink != 1 ||
      (s.st_mode & 0077) || (uint64_t)s.st_dev != dev ||
      (uint64_t)s.st_ino != ino || s.st_size < 0 ||
      (exact && (uint64_t)s.st_size != bytes)) {
    errno = ESTALE;
    return -1;
  }
  return 0;
}
static int check_source(void *v, int fd, const struct lfg_message *m) {
  (void)v;
  return inspect(fd, (uid_t)m->uid, m->source_dev, m->source_ino,
                 m->source_size, 1);
}
static int check_status(void *v, int fd, const struct lfg_message *m) {
  (void)v;
  return inspect(fd, (uid_t)m->uid, m->status_dev, m->status_ino, 0, 0);
}
static int validate_ready(void *v, const struct lfg_message *a,
                          const struct lfg_message *b) {
  struct fixture *f = v;
  (void)a;
  ++f->ready_calls;
  if (!strcmp(f->mode, "wrong-ready") || b->native_id[0] != 42 ||
      b->native_id[1] != 43) {
    errno = ESTALE;
    return -1;
  }
  return 0; /* test outcome, not a native mount claim */
}
static int native_absent(void *v, const struct lfg_message *m) {
  struct fixture *f = v;
  (void)m;
  ++f->absent_calls;
  if (!strcmp(f->mode, "native-error-first") && f->absent_calls == 1) {
    errno = ESTALE;
    return -1;
  }
  if (!strcmp(f->mode, "native-busy-first") && f->absent_calls == 1)
    return LFGR_BUSY;
  if (!strcmp(f->mode, "native-busy-final") && f->absent_calls == 2)
    return LFGR_BUSY;
  if (!strcmp(f->mode, "native-error-final") && f->absent_calls == 2) {
    errno = ESTALE;
    return -1;
  }
  return 0; /* test outcome, not a native mount claim */
}
static int flush_source(void *v, int fd, const struct lfg_message *m) {
  struct fixture *f = v;
  ++f->flush_calls;
  if (check_source(v, fd, m))
    return -1;
  if (f->source_sync_fail) {
    errno = EIO;
    return -1;
  }
  return barrier(fd);
}
static int close_source(void *v, int *fd, const struct lfg_message *m) {
  struct fixture *f = v;
  ++f->close_calls;
  if (check_source(v, *fd, m))
    return -1;
  if (!strcmp(f->mode, "false-close-success"))
    return 0; /* callback contract violation */
  int closing = *fd;
  *fd = -1;
  int rc = close(closing);
  if (f->source_close_fail) {
    errno = EIO;
    return -1;
  }
  return rc;
}
static int persist_status(void *v, int held, const struct lfg_message *m) {
  struct fixture *f = v;
  uint8_t bytes[512];
  size_t at = 0;
  ++f->persist_calls;
  if (f->guardian) {
    struct lfg_message request = f->guardian->identity, reply;
    request.kind = LFG_QUERY;
    request.state = LFG_REQUEST;
    CHECK(lfgr_reply(f->guardian, &request, &reply) < 0 && errno == EBUSY);
  }
  if (check_status(v, held, m))
    return -1;
  if (lfg_encode(bytes, sizeof bytes, m)) {
    errno = EPROTO;
    return -1;
  }
  int fd = fcntl(held, F_DUPFD_CLOEXEC, 32);
  if (fd < 0)
    return -1;
  int rc = 0, saved = 0;
  while (at < sizeof bytes) {
    if (f->status_write_fail) {
      rc = -1;
      saved = EIO;
      break;
    }
    ssize_t n = pwrite(fd, bytes + at, sizeof bytes - at, (off_t)at);
    if (n < 0 && errno == EINTR)
      continue;
    if (n <= 0) {
      rc = -1;
      saved = n ? errno : EIO;
      break;
    }
    at += (size_t)n;
  }
  if (!rc && ftruncate(fd, sizeof bytes)) {
    rc = -1;
    saved = errno;
  }
  if (!rc) {
    if (f->status_sync_fail) {
      rc = -1;
      saved = EIO;
    } else if (barrier(fd)) {
      rc = -1;
      saved = errno;
    }
  }
  if (!rc && check_status(v, fd, m)) {
    rc = -1;
    saved = errno;
  }
  if (close(fd)) {
    if (!rc)
      saved = errno;
    rc = -1;
  }
  if (!rc && check_status(v, held, m)) {
    rc = -1;
    saved = errno;
  }
  if (f->status_close_fail) {
    if (!rc)
      saved = EIO;
    rc = -1;
  }
  if (rc)
    errno = saved;
  return rc;
}
static const struct lfgr_ops ops = {check_source,  check_status, validate_ready,
                                    native_absent, flush_source, close_source,
                                    persist_status};
static void transfer(int fd, void *p, size_t n, int writing) {
  size_t at = 0;
  while (at < n) {
    ssize_t got = writing ? write(fd, (char *)p + at, n - at)
                          : read(fd, (char *)p + at, n - at);
    if (got < 0 && errno == EINTR)
      continue;
    CHECK(got > 0);
    at += (size_t)got;
  }
}
static void send_record(int fd, struct lfg_message *m) {
  uint8_t b[512];
  CHECK(!lfg_encode(b, sizeof b, m));
  transfer(fd, b, sizeof b, 1);
}
static void recv_record(int fd, struct lfg_message *m) {
  uint8_t b[512];
  transfer(fd, b, sizeof b, 0);
  CHECK(!lfg_decode(m, b, sizeof b));
}
static void capture_fixture(struct fixture *f, int source, int status) {
  struct stat s, t;
  CHECK(!fstat(source, &s));
  CHECK(!fstat(status, &t));
  f->source_dev = s.st_dev;
  f->source_ino = s.st_ino;
  f->status_dev = t.st_dev;
  f->status_ino = t.st_ino;
}
static int worker(const char *mode) {
  struct lfg_message m;
  recv_record(3, &m);
  struct fixture f = {.mode = mode};
  capture_fixture(&f, 4, 5);
  CHECK(!check_source(&f, 4, &m));
  CHECK(!check_status(&f, 5, &m));
  CHECK(fcntl(100, F_GETFD) < 0 && errno == EBADF);
  CHECK(!flock(4, LOCK_EX | LOCK_NB));
  m.kind = LFG_READY;
  m.state = LFG_RUNNING;
  m.native_id[0] = 42;
  m.native_id[1] = 43;
  ++m.sequence;
  send_record(3, &m);
  if (!strcmp(mode, "missing-drained"))
    return 0;
  if (!strcmp(mode, "worker-error-then-drained")) {
    struct lfg_message e = m;
    e.kind = LFG_ERROR;
    e.state = LFG_RETAINED;
    e.error = e.latched_error = EIO;
    ++e.sequence;
    send_record(3, &e);
    m.sequence = e.sequence;
  }
  m.kind = LFG_DRAINED;
  m.state = LFG_WORKER_DRAINED;
  m.completed_checks = LFG_C_ALL;
  ++m.sequence;
  send_record(3, &m);
  if (!strcmp(mode, "replayed-drained"))
    send_record(3, &m);
  if (!strcmp(mode, "signal-worker")) {
    raise(SIGTERM);
    return 98;
  }
  if (!strcmp(mode, "visible-clean-exit1")) {
    m.kind = LFG_REPLY;
    m.state = LFG_RELEASED_CLEAN;
    m.flags = LFG_F_ALL;
    m.wait_kind = LFG_WAIT_EXITED;
    f.status_sync_fail = 1;
    CHECK(persist_status(&f, 5, &m) < 0 && errno == EIO);
    f.status_write_fail = 1;
    m.kind = LFG_ERROR;
    m.state = LFG_RETAINED;
    m.error = m.latched_error = EIO;
    CHECK(persist_status(&f, 5, &m) < 0 && errno == EIO);
    return 1;
  }
  return 0;
}
static struct lfg_message identity(int source, int status, pid_t child) {
  struct lfg_message m = {0};
  struct stat s, t;
  CHECK(!fstat(source, &s));
  CHECK(!fstat(status, &t));
  m.kind = LFG_BIND;
  m.state = LFG_STARTING;
  m.required_checks = LFG_C_ALL;
  memset(m.session_nonce, 1, 32);
  memset(m.request_nonce, 2, 16);
  memset(m.boot_id, 3, 16);
  memset(m.component_sha256, 4, 32);
  m.uid = getuid();
  m.guardian_pid = getpid();
  m.guardian_birth_sec = 1;
  m.worker_pid = child;
  m.worker_birth_sec = 2;
  m.status_dev = t.st_dev;
  m.status_ino = t.st_ino;
  m.source_dev = s.st_dev;
  m.source_ino = s.st_ino;
  m.source_size = (uint64_t)s.st_size;
  m.source_kind = LFG_SOURCE_REGULAR;
  m.filesystem = 1;
  m.sequence = 1;
  return m;
}
static void run_case(const char *self, const char *root, const char *mode) {
  char dir[PATH_MAX], source[PATH_MAX], statuspath[PATH_MAX];
  CHECK(snprintf(dir, sizeof dir, "%s/%s", root, mode) < (int)sizeof dir);
  CHECK(!mkdir(dir, 0700));
  CHECK(snprintf(source, sizeof source, "%s/source", dir) < (int)sizeof source);
  CHECK(snprintf(statuspath, sizeof statuspath, "%s/status", dir) <
        (int)sizeof statuspath);
  int s = open(source, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
               0600),
      t = open(statuspath, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
               0600);
  CHECK(s >= 0 && t >= 0);
  CHECK(!ftruncate(s, 4096));
  CHECK(!flock(s, LOCK_EX | LOCK_NB));
  CHECK(!flock(t, LOCK_EX | LOCK_NB));
  struct fixture f = {.mode = mode};
  capture_fixture(&f, s, t);
  int pair[2];
  CHECK(!socketpair(AF_UNIX, SOCK_STREAM, 0, pair));
  for (int i = 0; i < 2; i++)
    CHECK(!fcntl(pair[i], F_SETFD, FD_CLOEXEC));
  int src[3] = {fcntl(pair[1], F_DUPFD_CLOEXEC, 32),
                fcntl(s, F_DUPFD_CLOEXEC, 32), fcntl(t, F_DUPFD_CLOEXEC, 32)};
  for (int i = 0; i < 3; i++)
    CHECK(src[i] >= 32);
  int sentinel = fcntl(s, F_DUPFD, 100);
  CHECK(sentinel == 100);
  CHECK(!(fcntl(sentinel, F_GETFD) & FD_CLOEXEC));
  posix_spawn_file_actions_t actions;
  posix_spawnattr_t attr;
  CHECK(!posix_spawn_file_actions_init(&actions));
  CHECK(!posix_spawnattr_init(&attr));
#ifdef __APPLE__
  CHECK(!posix_spawnattr_setflags(&attr, POSIX_SPAWN_CLOEXEC_DEFAULT));
#else
#ifndef __FreeBSD__
#error Native Mac or FreeBSD control required
#endif
#endif
  CHECK(!posix_spawn_file_actions_adddup2(&actions, 1, 1));
  CHECK(!posix_spawn_file_actions_adddup2(&actions, 2, 2));
  for (int i = 0; i < 3; i++)
    CHECK(!posix_spawn_file_actions_adddup2(&actions, src[i], 3 + i));
#ifdef __FreeBSD__
  CHECK(!posix_spawn_file_actions_addclosefrom_np(&actions, 6));
#endif
  char *args[] = {(char *)self, "--worker", (char *)mode, NULL};
  pid_t child;
  CHECK(!posix_spawn(&child, self, &actions, &attr, args, environ));
  CHECK(!posix_spawn_file_actions_destroy(&actions));
  CHECK(!posix_spawnattr_destroy(&attr));
  CHECK(!close(sentinel));
  for (int i = 0; i < 3; i++)
    CHECK(!close(src[i]));
  CHECK(!close(pair[1]));
  struct lfg_message bind = identity(s, t, child);
  struct lfgr_guardian g;
  if (!strcmp(mode, "normal")) {
    struct lfgr_guardian untouched;
    memset(&g, 0xa5, sizeof g);
    untouched = g;
    CHECK(lfgr_init(&g, &bind, child, s, t, NULL, &f) < 0);
    CHECK(!memcmp(&g, &untouched, sizeof g));
    struct lfg_message bad = bind;
    bad.worker_pid++;
    CHECK(lfgr_init(&g, &bad, child, s, t, &ops, &f) < 0);
    CHECK(lfgr_init(&g, &bind, child, s, s, &ops, &f) < 0);
    bad = bind;
    memset(bad.request_nonce, 0, sizeof bad.request_nonce);
    CHECK(lfgr_init(&g, &bad, child, s, t, &ops, &f) < 0);
    CHECK(fcntl(s, F_GETFD) >= 0 && fcntl(t, F_GETFD) >= 0);
  }
  CHECK(!lfgr_init(&g, &bind, child, s, t, &ops, &f));
  f.guardian = &g;
  CHECK(lfgr_reap(&g, 1) == 0); /* child still blocked awaiting BIND */
  send_record(pair[0], &bind);
  CHECK(lfgr_release(&g) == LFGR_BUSY);
  CHECK(f.absent_calls == 0 && f.flush_calls == 0 && f.close_calls == 0);
  struct lfg_message m;
  recv_record(pair[0], &m);
  int badready = !strcmp(mode, "wrong-ready");
  CHECK((lfgr_worker_record(&g, &m) < 0) == badready);
  if (strcmp(mode, "missing-drained")) {
    if (!strcmp(mode, "worker-error-then-drained")) {
      recv_record(pair[0], &m);
      CHECK(lfgr_worker_record(&g, &m) < 0);
    }
    recv_record(pair[0], &m);
    int failed = badready || !strcmp(mode, "worker-error-then-drained");
    CHECK((lfgr_worker_record(&g, &m) < 0) == failed);
    if (!strcmp(mode, "replayed-drained")) {
      recv_record(pair[0], &m);
      CHECK(lfgr_worker_record(&g, &m) < 0);
    }
  }
  CHECK(lfgr_reap(&g, 0) == 1);
  CHECK(g.wait_valid);
  int independent = open(statuspath, O_RDWR | O_NOFOLLOW | O_CLOEXEC);
  CHECK(independent >= 0);
  CHECK(flock(independent, LOCK_EX | LOCK_NB) < 0 &&
        (errno == EWOULDBLOCK || errno == EAGAIN));
  CHECK(!close(
      independent)); /* independent status lock is custody, not wait proof */
  if (!strcmp(mode, "missing-drained"))
    CHECK(lfgr_channel_closed(&g) < 0);
  else
    CHECK((lfgr_channel_closed(&g) < 0) == (g.sticky_error != 0));
  f.status_sync_fail = !strcmp(mode, "status-sync-error");
  f.source_sync_fail = !strcmp(mode, "source-sync-error");
  f.source_close_fail = !strcmp(mode, "source-close-error");
  f.status_close_fail = !strcmp(mode, "status-close-error");
  int good = !strcmp(mode, "normal") || !strcmp(mode, "native-busy-first");
  int rc = lfgr_release(&g);
  if (!strcmp(mode, "native-busy-first")) {
    CHECK(rc == LFGR_BUSY);
    CHECK(!g.sticky_error && g.custody_fd >= 0);
    CHECK(!f.flush_calls && !f.close_calls && !f.persist_calls);
    rc = lfgr_release(&g);
  }
  CHECK(good ? rc == LFGR_OK : rc == LFGR_ERROR);
  CHECK(good ? g.released : !g.released);
  struct lfg_message request = g.identity, reply;
  request.kind = LFG_QUERY;
  request.state = LFG_REQUEST;
  request.sequence = 0;
  CHECK(!lfgr_reply(&g, &request, &reply));
  CHECK(lfg_clean_candidate(&reply) == good);
  if (!strcmp(mode, "visible-clean-exit1") ||
      !strcmp(mode, "status-sync-error") ||
      !strcmp(mode, "status-close-error") ||
      !strcmp(mode, "native-busy-final") ||
      !strcmp(mode, "native-error-final")) {
    uint8_t b[512];
    CHECK(pread(t, b, sizeof b, 0) == sizeof b);
    CHECK(!lfg_decode(&m, b, sizeof b));
    CHECK(lfg_clean_candidate(&m));
    CHECK(!lfg_clean_candidate(&reply));
  }
  if (good) {
    CHECK(f.close_calls == 1 && f.flush_calls == 1 && f.persist_calls == 1);
    request.request_nonce[0]++;
    CHECK(!lfgr_reply(&g, &request, &reply));
    CHECK(lfg_clean_candidate(&reply));
    CHECK(reply.request_nonce[0] == request.request_nonce[0]);
    CHECK(lfgr_release(&g) == 0);
    CHECK(f.close_calls == 1 && f.persist_calls == 1);
    request.source_ino++;
    CHECK(lfgr_reply(&g, &request, &reply) < 0);
    request.source_ino--;
    CHECK(!g.sticky_error);
    request.native_id[0]++;
    CHECK(lfgr_reply(&g, &request, &reply) < 0);
    request.native_id[0]--;
    CHECK(!g.sticky_error);
    request.kind = LFG_RETIRE;
    CHECK(!lfgr_reply(&g, &request, &reply));
    CHECK(reply.state == LFG_RETIRED && g.retired);
    uint64_t seq = reply.sequence;
    request.request_nonce[0]++;
    CHECK(!lfgr_reply(&g, &request, &reply));
    CHECK(reply.state == LFG_RETIRED && reply.sequence == seq);
  } else {
    int error = g.sticky_error;
    f.source_sync_fail = f.status_sync_fail = f.source_close_fail =
        f.status_close_fail = 0;
    CHECK(lfgr_release(&g) == LFGR_ERROR && g.sticky_error == error);
    CHECK(!lfgr_reply(&g, &request, &reply));
    CHECK(!lfg_clean_candidate(&reply));
    request.kind = LFG_RETIRE;
    CHECK(lfgr_reply(&g, &request, &reply) < 0);
  }
  if (g.custody_fd >= 0)
    CHECK(!close(g.custody_fd));
  CHECK(!close(pair[0]));
  CHECK(!close(t));
  printf("{\"case\":\"%s\",\"actual_wait_kind\":%u,\"actual_wait_value\":%u,"
         "\"positive\":%s,\"sticky\":%d,\"native_check_calls\":%d}\n",
         mode, g.current.wait_kind, g.current.wait_value,
         good ? "true" : "false", g.sticky_error, f.absent_calls);
}
int main(int argc, char **argv) {
  if (argc == 3 && !strcmp(argv[1], "--worker"))
    return worker(argv[2]);
  CHECK(argc == 2);
  CHECK(getuid() != 0 && getuid() == geteuid());
  struct stat s;
  CHECK(!lstat(argv[1], &s) && S_ISDIR(s.st_mode) && s.st_uid == getuid() &&
        !(s.st_mode & 0077));
  char self[PATH_MAX];
  CHECK(realpath(argv[0], self));
  const char *cases[] = {"normal",
                         "visible-clean-exit1",
                         "status-sync-error",
                         "source-sync-error",
                         "source-close-error",
                         "status-close-error",
                         "native-error-first",
                         "native-busy-first",
                         "native-busy-final",
                         "native-error-final",
                         "wrong-ready",
                         "replayed-drained",
                         "missing-drained",
                         "signal-worker",
                         "worker-error-then-drained",
                         "false-close-success"};
  for (size_t i = 0; i < sizeof cases / sizeof cases[0]; i++)
    run_case(self, argv[1], cases[i]);
  printf("{\"result\":\"pass\",\"checks\":%u,\"scope\":\"common POSIX state "
         "and actual child wait; native mount outcomes are control inputs, no "
         "kernel/FUSE/device execution\"}\n",
         checks);
  return 0;
}
