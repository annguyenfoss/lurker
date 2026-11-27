/* SPDX-License-Identifier: MIT
 * Native supervisor contract controls; no FUSE, LKL, mount or device I/O.
 */
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
static int injected_status_sync, injected_source_sync, injected_status_write;
static int injected_source_close, injected_status_close;
static dev_t status_dev, source_dev;
static ino_t status_ino, source_ino;
static int is_inode(int fd, dev_t dev, ino_t ino) {
  struct stat st;
  return !fstat(fd, &st) && st.st_dev == dev && st.st_ino == ino;
}
static int control_fsync(int fd) {
  if ((injected_status_sync && is_inode(fd, status_dev, status_ino)) ||
      (injected_source_sync && is_inode(fd, source_dev, source_ino))) {
    errno = EIO;
    return -1;
  }
  return fsync(fd);
}
static ssize_t control_pwrite(int fd, const void *p, size_t n, off_t off) {
  if (injected_status_write && is_inode(fd, status_dev, status_ino)) {
    errno = EIO;
    return -1;
  }
  return pwrite(fd, p, n, off);
}
static int control_close(int fd) {
  int fail = (injected_source_close && is_inode(fd, source_dev, source_ino)) ||
             (injected_status_close && is_inode(fd, status_dev, status_ino));
  int rc = close(fd);
  if (fail) {
    errno = EIO;
    return -1;
  }
  return rc;
}
#define LG_TRACE(...) fprintf(stderr, __VA_ARGS__)
#define LG_FSYNC control_fsync
#define LG_PWRITE control_pwrite
#define LG_CLOSE control_close
#include "../lkl-guardian.h"
#include <sys/file.h>
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
  CHECK(lfg_encode(b, sizeof b, m) == 0);
  transfer(fd, b, sizeof b, 1);
}
static void recv_record(int fd, struct lfg_message *m) {
  uint8_t b[512];
  transfer(fd, b, sizeof b, 0);
  CHECK(lfg_decode(m, b, sizeof b) == 0);
}
static void inode_globals(int source, int status) {
  struct stat s, t;
  CHECK(!fstat(source, &s));
  CHECK(!fstat(status, &t));
  source_dev = s.st_dev;
  source_ino = s.st_ino;
  status_dev = t.st_dev;
  status_ino = t.st_ino;
}
static int worker(const char *mode) {
  struct lfg_message m;
  recv_record(3, &m);
  inode_globals(4, 5);
  CHECK(fcntl(100, F_GETFD) < 0 && errno == EBADF);
  CHECK(!flock(4, LOCK_EX | LOCK_NB)); /* same OFD, not a terminal proof */
  CHECK(!lg_regular_fd(4, getuid(), m.source_dev, m.source_ino, m.source_size,
                       1));
  CHECK(!lg_regular_fd(5, getuid(), m.status_dev, m.status_ino, 0, 0));
  ++m.sequence;
  m.kind = LFG_READY;
  m.state = LFG_RUNNING;
  m.native_id[0] = 42;
  m.native_id[1] = 43;
  send_record(3, &m);
  ++m.sequence;
  m.kind = LFG_DRAINED;
  m.state = LFG_WORKER_DRAINED;
  m.completed_checks = LFG_C_ALL;
  send_record(3, &m);
  if (!strcmp(mode, "visible-clean-exit1")) {
    m.kind = LFG_REPLY;
    m.state = LFG_RELEASED_CLEAN;
    m.flags = LFG_F_ALL;
    m.wait_kind = LFG_WAIT_EXITED;
    m.sequence = 9;
    injected_status_sync = 1;
    CHECK(lg_write_status(5, &m) < 0 && errno == EIO);
    injected_status_write = 1;
    m.kind = LFG_ERROR;
    m.state = LFG_RETAINED;
    m.error = m.latched_error = EIO;
    CHECK(lg_write_status(5, &m) < 0 && errno == EIO);
    return 1;
  }
  return 0;
}
static struct lfg_message identity(int source, int status) {
  struct lfg_message m = {0};
  struct stat s, t;
  struct lg_process p;
  CHECK(!fstat(source, &s));
  CHECK(!fstat(status, &t));
  CHECK(!lg_process_snapshot(getpid(), &p));
  m.kind = LFG_BIND;
  m.state = LFG_STARTING;
  m.required_checks = LFG_C_ALL;
  memset(m.session_nonce, 1, 32);
  memset(m.request_nonce, 2, 16);
  memset(m.boot_id, 3, 16);
  memset(m.component_sha256, 4, 32);
  m.uid = getuid();
  m.guardian_pid = getpid();
  m.guardian_birth_sec = p.sec;
  m.guardian_birth_usec = p.usec;
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
static int rpc_client(const char *path, int good) {
  struct lfg_message request, reply;
  recv_record(3, &request);
  CHECK(lg_call(path, &request, &reply, lg_milliseconds() + 3000) < 0);
  request.request_nonce[0]++;
  CHECK(!lg_call(path, &request, &reply, lg_milliseconds() + 3000));
  CHECK(lfg_clean_candidate(&reply) == good);
  request.request_nonce[0]++;
  CHECK(lg_call(path, &request, &reply, lg_milliseconds() + 3000) <
        0); /* wrong echo */
  request.request_nonce[0]++;
  CHECK(lg_call(path, &request, &reply, lg_milliseconds() + 3000) <
        0); /* absent guardian */
  return 0;
}
static void rpc_controls(const char *self, const char *path,
                         struct lfgr_guardian *g, int good) {
  struct sockaddr_un address = {.sun_family = AF_UNIX};
  CHECK(strlen(path) < sizeof address.sun_path);
  strcpy(address.sun_path, path);
  address.sun_len = sizeof address;
  int listener = socket(AF_UNIX, SOCK_STREAM, 0);
  CHECK(listener >= 0);
  CHECK(!fcntl(listener, F_SETFD, FD_CLOEXEC));
  mode_t mask = umask(0077);
  CHECK(!bind(listener, (struct sockaddr *)&address, sizeof address));
  umask(mask);
  CHECK(!listen(listener, 4));
  int config[2];
  CHECK(!pipe(config));
  int childfd = fcntl(config[0], F_DUPFD_CLOEXEC, 32);
  CHECK(childfd >= 32);
  posix_spawn_file_actions_t actions;
  posix_spawnattr_t attr;
  CHECK(!posix_spawn_file_actions_init(&actions));
  CHECK(!posix_spawnattr_init(&attr));
  CHECK(!posix_spawnattr_setflags(&attr, POSIX_SPAWN_CLOEXEC_DEFAULT));
  CHECK(!posix_spawn_file_actions_adddup2(&actions, 1, 1));
  CHECK(!posix_spawn_file_actions_adddup2(&actions, 2, 2));
  CHECK(!posix_spawn_file_actions_adddup2(&actions, childfd, 3));
  char *args[] = {(char *)self, "--rpc-client", (char *)path, good ? "1" : "0",
                  NULL};
  pid_t child;
  CHECK(!posix_spawn(&child, self, &actions, &attr, args, environ));
  CHECK(!posix_spawn_file_actions_destroy(&actions));
  CHECK(!posix_spawnattr_destroy(&attr));
  CHECK(!close(childfd));
  CHECK(!close(config[0]));
  struct lg_process expected;
  CHECK(!lg_process_snapshot(child, &expected));
  struct lfg_message request = g->identity, reply;
  request.kind = LFG_QUERY;
  request.state = LFG_REQUEST;
  send_record(config[1], &request);
  CHECK(!close(config[1]));
  for (int i = 0; i < 3; i++) {
    struct pollfd p = {.fd = listener, .events = POLLIN};
    CHECK(poll(&p, 1, 3000) == 1);
    int fd = accept(listener, NULL, NULL);
    CHECK(fd >= 0);
    struct lg_process peer;
    CHECK(!lg_peer(fd, &peer));
    CHECK(lg_same_process(&peer, expected.pid, expected.sec, expected.usec));
    CHECK(!lg_receive(fd, &request, lg_milliseconds() + 3000));
    CHECK(!lfgr_reply(g, &request, &reply));
    if (i == 2) {
      CHECK(!close(listener));
      listener = -1;
      CHECK(!unlink(path));
      reply.request_nonce[0] ^= 1;
    }
    if (i)
      CHECK(!lg_send(fd, &reply, lg_milliseconds() + 3000));
    CHECK(!close(fd)); /* first reply deliberately lost, state remains live */
  }
  int status;
  CHECK(waitpid(child, &status, 0) == child);
  CHECK(WIFEXITED(status) && WEXITSTATUS(status) == 0);
}
struct native_control {
  int result;
};
static int controlled_ready(void *v, const struct lfg_message *expected,
                            const struct lfg_message *ready) {
  (void)v;
  (void)expected;
  if (ready->native_id[0] != 42 || ready->native_id[1] != 43) {
    errno = ESTALE;
    return -1;
  }
  return 0; /* explicit native identity outcome, no real filesystem mount */
}
static int controlled_absent(void *v, const struct lfg_message *m) {
  struct native_control *control = v;
  (void)m;
  if (control->result < 0) {
    errno = ESTALE;
    return -1;
  }
  return control->result;
}
static void run_case(const char *self, const char *root, const char *mode) {
  char dir[1024], source[1100], status[1100];
  snprintf(dir, sizeof dir, "%s/%s", root, mode);
  CHECK(!mkdir(dir, 0700));
  snprintf(source, sizeof source, "%s/source", dir);
  snprintf(status, sizeof status, "%s/status", dir);
  int sourcefd =
      open(source, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
  int statusfd =
      open(status, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0600);
  CHECK(sourcefd >= 0 && statusfd >= 0);
  CHECK(!ftruncate(sourcefd, 4096));
  CHECK(!flock(sourcefd, LOCK_EX | LOCK_NB));
  CHECK(!flock(statusfd, LOCK_EX | LOCK_NB));
  inode_globals(sourcefd, statusfd);
  int pair[2];
  CHECK(!socketpair(AF_UNIX, SOCK_STREAM, 0, pair));
  for (int i = 0; i < 2; i++)
    CHECK(!fcntl(pair[i], F_SETFD, FD_CLOEXEC));
  /* Keep mapping source descriptors away from destination3/4/5. */
  int src[3] = {fcntl(pair[1], F_DUPFD_CLOEXEC, 32),
                fcntl(sourcefd, F_DUPFD_CLOEXEC, 32),
                fcntl(statusfd, F_DUPFD_CLOEXEC, 32)};
  for (int i = 0; i < 3; i++)
    CHECK(src[i] >= 32);
  int sentinel = fcntl(sourcefd, F_DUPFD, 100);
  CHECK(sentinel == 100);
  CHECK(!(fcntl(sentinel, F_GETFD) & FD_CLOEXEC));
  posix_spawn_file_actions_t actions;
  posix_spawnattr_t attr;
  CHECK(!posix_spawn_file_actions_init(&actions));
  CHECK(!posix_spawnattr_init(&attr));
  CHECK(!posix_spawnattr_setflags(&attr, POSIX_SPAWN_CLOEXEC_DEFAULT));
  CHECK(!posix_spawn_file_actions_adddup2(&actions, 1, 1));
  CHECK(!posix_spawn_file_actions_adddup2(&actions, 2, 2));
  for (int i = 0; i < 3; i++)
    CHECK(!posix_spawn_file_actions_adddup2(&actions, src[i], 3 + i));
  char *args[] = {(char *)self, "--worker", (char *)mode, NULL};
  pid_t child;
  CHECK(!posix_spawn(&child, self, &actions, &attr, args, environ));
  CHECK(!posix_spawn_file_actions_destroy(&actions));
  CHECK(!posix_spawnattr_destroy(&attr));
  CHECK(!close(sentinel));
  for (int i = 0; i < 3; i++)
    CHECK(!close(src[i]));
  CHECK(!close(pair[1]));
  struct lfgr_guardian g;
  struct lfg_message context = identity(sourcefd, statusfd);
  struct lg_process p;
  CHECK(!lg_process_snapshot(child, &p));
  context.worker_pid = child;
  context.worker_birth_sec = p.sec;
  context.worker_birth_usec = p.usec;
  struct native_control control = {0};
  struct lg_native_binding binding = {&control, controlled_ready,
                                      controlled_absent};
  CHECK(!lg_runtime_init(&g, &context, child, sourcefd, statusfd, &binding));
  send_record(pair[0], &g.identity);
  struct lfg_message m;
  recv_record(pair[0], &m);
  CHECK(!lfgr_worker_record(&g, &m));
  recv_record(pair[0], &m);
  CHECK(!lfgr_worker_record(&g, &m));
  CHECK(lfgr_release(&g) == LFGR_BUSY); /* no wait: must not release */
  CHECK(g.custody_fd >= 0 && !g.released);
  CHECK(lfgr_reap(&g, 0) == 1);
  CHECK(g.wait_valid);
  if (!g.sticky_error) {
    control.result = LFGR_BUSY;
    CHECK(lfgr_release(&g) == LFGR_BUSY);
    control.result = 0;
    CHECK(!g.sticky_error && !g.released);
  }
  if (!strcmp(mode, "status-sync-error"))
    injected_status_sync = 1;
  if (!strcmp(mode, "source-sync-error"))
    injected_source_sync = 1;
  if (!strcmp(mode, "source-close-error"))
    injected_source_close = 1;
  if (!strcmp(mode, "status-close-error"))
    injected_status_close = 1;
  int good = !strcmp(mode, "normal");
  if (!strcmp(mode, "native-check-error"))
    control.result = -1;
  int rc = lfgr_release(&g);
  CHECK(good ? rc == 0 : rc < 0);
  CHECK(good ? g.released : !g.released);
  struct lfg_message request = g.identity, reply;
  request.kind = LFG_QUERY;
  request.state = LFG_REQUEST;
  request.sequence = 0;
  CHECK(!lfgr_reply(&g, &request, &reply));
  CHECK(lfg_clean_candidate(&reply) == good);
  if (!strcmp(mode, "visible-clean-exit1") ||
      !strcmp(mode, "status-sync-error") ||
      !strcmp(mode, "status-close-error")) {
    uint8_t b[512];
    CHECK(pread(statusfd, b, sizeof b, 0) == sizeof b);
    CHECK(!lfg_decode(&m, b, sizeof b));
    CHECK(lfg_clean_candidate(&m));
    CHECK(!lfg_clean_candidate(&reply));
  }
  char sockpath[1100];
  snprintf(sockpath, sizeof sockpath, "%s/s", dir);
  rpc_controls(self, sockpath, &g, good);
  if (good) {
    /* Lost first response: no file re-read, fresh nonce replays live state. */
    request.request_nonce[0]++;
    CHECK(!lfgr_reply(&g, &request, &reply));
    CHECK(lfg_clean_candidate(&reply));
    CHECK(reply.request_nonce[0] == request.request_nonce[0]);
    request.native_id[0]++;
    CHECK(lfgr_reply(&g, &request, &reply) < 0);
    request.native_id[0]--;
    request.kind = LFG_RETIRE;
    CHECK(!lfgr_reply(&g, &request, &reply));
    CHECK(reply.state == LFG_RETIRED);
  }
  injected_status_sync = injected_source_sync = injected_status_write =
      injected_source_close = injected_status_close = 0;
  if (g.custody_fd >= 0)
    CHECK(!close(g.custody_fd));
  CHECK(!close(pair[0]));
  CHECK(!close(statusfd));
  printf("{\"case\":\"%s\",\"actual_wait_exit\":%d,\"positive\":%s,\"sticky\":%"
         "d}\n",
         mode, WIFEXITED(g.wait_status) ? WEXITSTATUS(g.wait_status) : -1,
         good ? "true" : "false", g.sticky_error);
}
int main(int argc, char **argv) {
  if (argc == 3 && !strcmp(argv[1], "--worker"))
    return worker(argv[2]);
  if (argc == 4 && !strcmp(argv[1], "--rpc-client"))
    return rpc_client(argv[2], !strcmp(argv[3], "1"));
  CHECK(argc == 2);
  CHECK(getuid() != 0 && getuid() == geteuid());
  struct stat st;
  CHECK(!lstat(argv[1], &st) && S_ISDIR(st.st_mode) && st.st_uid == getuid() &&
        !(st.st_mode & 0077));
  char self[1024];
  CHECK(realpath(argv[0], self));
  const char *cases[] = {"normal",
                         "visible-clean-exit1",
                         "status-sync-error",
                         "source-sync-error",
                         "source-close-error",
                         "status-close-error",
                         "native-check-error"};
  for (size_t i = 0; i < sizeof cases / sizeof cases[0]; i++)
    run_case(self, argv[1], cases[i]);
  printf("{\"result\":\"pass\",\"checks\":%u,\"scope\":\"native "
         "owned-child/status/custody and live RPC; no FUSE acceptance\"}\n",
         checks);
  return 0;
}
