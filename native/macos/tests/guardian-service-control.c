/* SPDX-License-Identifier: MIT
 * Native service/dummy-worker controls only. No LKL, FUSE, mounts, crypto
 * providers, storage devices or installation. All sources are owned 4KiB files.
 */
#include <errno.h>
#include <sys/stat.h>
#include <unistd.h>
static int inject_duplicate_close;
static dev_t injected_source_device;
static ino_t injected_source_inode;
static int controlled_close(int fd) {
  struct stat st;
  int fail = inject_duplicate_close && fd >= 16 && !fstat(fd, &st) &&
             st.st_dev == injected_source_device &&
             st.st_ino == injected_source_inode;
  int rc = close(fd);
  if (fail) {
    inject_duplicate_close = 0;
    errno = EIO;
    return -1;
  }
  return rc;
}
#define LG_CLOSE controlled_close
#include "../guardian-service.h"
#include <signal.h>
#include <sys/random.h>
static unsigned checks;
#define CHECK(x)                                                               \
  do {                                                                         \
    ++checks;                                                                  \
    if (!(x)) {                                                                \
      fprintf(stderr, "line%u check%u %s errno=%d\n", __LINE__, checks, #x,    \
              errno);                                                          \
      exit(90);                                                                \
    }                                                                          \
  } while (0)
struct control_native {
  const char *mode;
  int source;
};
static int byte_at(int fd) {
  unsigned char b = 0;
  CHECK(pread(fd, &b, 1, 0) == 1);
  return b;
}
static void set_byte(const char *dir, unsigned char b) {
  char p[PATH_MAX];
  snprintf(p, sizeof p, "%s/source", dir);
  int fd = open(p, O_RDWR | O_NOFOLLOW | O_CLOEXEC);
  CHECK(fd >= 0);
  CHECK(pwrite(fd, &b, 1, 0) == 1);
  CHECK(!fsync(fd));
  CHECK(!close(fd));
}
static int ready(void *v, const struct lfg_message *a,
                 const struct lfg_message *b) {
  (void)v;
  return b->native_id[0] == 42 && b->native_id[1] == 43 &&
                 b->native_id[2] == a->source_dev &&
                 b->native_id[3] == a->source_ino
             ? 0
             : (errno = ESTALE, -1);
}
static int absent(void *v, const struct lfg_message *m) {
  struct control_native *c = v;
  (void)m;
  return !strcmp(c->mode, "busy") && byte_at(c->source) == 0
             ? LFGR_BUSY
             : 0; /* explicit control outcome, not mount evidence */
}
static int persist(void *v, int fd, const struct lfg_message *m) {
  struct control_native *c = v;
  if (!strcmp(c->mode, "forged-clean-exit1") && byte_at(c->source) >= 2) {
    errno = EIO;
    return -1;
  }
  return lg_persist_status(NULL, fd, m);
}
static const struct lfgr_ops control_ops = {
    lg_check_source, lg_check_status, ready,  absent,
    lg_flush_source, lg_close_source, persist};
static void make_paths(const char *dir, char source[PATH_MAX],
                       char status[PATH_MAX], char socket[PATH_MAX]) {
  CHECK(snprintf(source, PATH_MAX, "%s/source", dir) < PATH_MAX);
  CHECK(snprintf(status, PATH_MAX, "%s/status", dir) < PATH_MAX);
  CHECK(snprintf(socket, PATH_MAX, "%s/s", dir) < PATH_MAX);
}
static void progress(struct lfg_message *m, uint16_t kind, uint16_t state) {
  m->kind = kind;
  m->state = state;
  ++m->sequence;
  CHECK(!lg_send(3, m, lg_milliseconds() + 500));
}
static int wait_marker(unsigned char minimum) {
  uint64_t end = lg_milliseconds() + 1500;
  while (lg_milliseconds() < end) {
    if (byte_at(4) >= minimum)
      return 0;
    usleep(5000);
  }
  return -1;
}
static int dummy_worker(const char *mode) {
  /* First application actions: inherited3/4 become CLOEXEC, with no status5. */
  CHECK(!fcntl(3, F_SETFD, FD_CLOEXEC));
  CHECK(!fcntl(4, F_SETFD, FD_CLOEXEC));
  CHECK(fcntl(5, F_GETFD) < 0 && errno == EBADF);
  CHECK(fcntl(100, F_GETFD) < 0 && errno == EBADF);
  struct stat channel;
  CHECK(!fstat(3, &channel) && S_ISSOCK(channel.st_mode));
  char ignored;
  CHECK(read(0, &ignored, 1) == 0);
  struct lgs_executable self;
  CHECK(!lgs_self(&self));
  struct lfg_message m;
  CHECK(!lg_receive(3, &m, lg_milliseconds() + 500));
  struct lg_process own, peer;
  CHECK(!lg_process_snapshot(getpid(), &own));
  CHECK(!lg_peer(3, &peer));
  CHECK(m.kind == LFG_BIND && m.worker_pid == (uint64_t)getpid());
  CHECK(lg_same_process(&own, m.worker_pid, m.worker_birth_sec,
                        m.worker_birth_usec));
  CHECK(lg_same_process(&peer, m.guardian_pid, m.guardian_birth_sec,
                        m.guardian_birth_usec));
  CHECK(!memcmp(m.component_sha256, self.sha256, 32));
  CHECK(!lg_check_source(NULL, 4, &m));
  CHECK(!flock(4, LOCK_EX | LOCK_NB));
  CHECK(!close(self.fd));
  fprintf(stderr,
          "DUMMY_BOUND worker=%d channel3=socket source4=owned status5=absent "
          "sentinel100=absent birth=matched component=matched\n",
          getpid());
  m.native_id[0] = 42;
  m.native_id[1] = 43;
  m.native_id[2] = m.source_dev;
  m.native_id[3] = m.source_ino;
  if (!strcmp(mode, "startup-hang") && wait_marker(1))
    return 91;
  if (!strcmp(mode, "partial-hang") || !strcmp(mode, "partial-eof")) {
    uint8_t bytes[512];
    m.kind = LFG_READY;
    m.state = LFG_RUNNING;
    ++m.sequence;
    CHECK(!lfg_encode(bytes, sizeof bytes, &m));
    CHECK(send(3, bytes, 17, 0) == 17);
    if (!strcmp(mode, "partial-eof"))
      return 0;
    if (wait_marker(1))
      return 91;
    CHECK(!lg_socket_io(3, bytes + 17, 495, 1, lg_milliseconds() + 500));
  } else
    progress(&m, LFG_READY, LFG_RUNNING);
  if (wait_marker(1))
    return 91;
  m.completed_checks = LFG_C_ALL;
  progress(&m, LFG_DRAINED, LFG_WORKER_DRAINED);
  if (!strcmp(mode, "forged-clean-exit1")) {
    if (wait_marker(2))
      return 91;
    return 1;
  }
  return 0;
}
static struct lfg_message context(int source, int status, pid_t worker,
                                  const struct lgs_executable *self) {
  struct lfg_message m = {0};
  struct stat s, t;
  struct lg_process owner, child;
  struct timeval boot;
  size_t n = sizeof boot;
  CHECK(!fstat(source, &s));
  CHECK(!fstat(status, &t));
  CHECK(!lg_process_snapshot(getpid(), &owner));
  CHECK(!lg_process_snapshot(worker, &child));
  CHECK(!sysctlbyname("kern.boottime", &boot, &n, NULL, 0) && n == sizeof boot);
  m.kind = LFG_BIND;
  m.state = LFG_STARTING;
  m.source_kind = LFG_SOURCE_REGULAR;
  m.filesystem = 0x54455354;
  m.required_checks = LFG_C_ALL;
  m.sequence = 1;
  CHECK(!getentropy(m.session_nonce, 32));
  CHECK(!getentropy(m.request_nonce, 16));
  lfg_put64(m.boot_id, (uint64_t)boot.tv_sec);
  lfg_put64(m.boot_id + 8, (uint64_t)boot.tv_usec);
  m.uid = getuid();
  m.guardian_pid = getpid();
  m.guardian_birth_sec = owner.sec;
  m.guardian_birth_usec = owner.usec;
  m.worker_pid = worker;
  m.worker_birth_sec = child.sec;
  m.worker_birth_usec = child.usec;
  m.source_dev = s.st_dev;
  m.source_ino = s.st_ino;
  m.source_size = (uint64_t)s.st_size;
  m.status_dev = t.st_dev;
  m.status_ino = t.st_ino;
  memcpy(m.component_sha256, self->sha256, 32);
  return m;
}
static int service(const char *dir, const char *mode) {
  char source[PATH_MAX], status[PATH_MAX], socket[PATH_MAX];
  make_paths(dir, source, status, socket);
  int s = open(source, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
               0600),
      t = open(status, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
               0600);
  CHECK(s >= 0 && t >= 0);
  CHECK(!ftruncate(s, 4096));
  CHECK(!flock(s, LOCK_EX | LOCK_NB));
  CHECK(!flock(t, LOCK_EX | LOCK_NB));
  struct lgs_executable self;
  CHECK(!lgs_self(&self));
  int sentinel = fcntl(t, F_DUPFD, 100);
  CHECK(sentinel == 100);
  CHECK(!(fcntl(sentinel, F_GETFD) & FD_CLOEXEC));
  struct lgs_service svc = {0};
  CHECK(!lgs_listen(&svc, dir, "s"));
  char *args[] = {self.path, "--dummy", (char *)mode, NULL};
  pid_t worker = 0;
  int channel = -1;
  if (!strcmp(mode, "spawn-close-error")) {
    struct stat st;
    CHECK(!fstat(s, &st));
    injected_source_device = st.st_dev;
    injected_source_inode = st.st_ino;
    inject_duplicate_close = 1;
  }
  int spawned = lgs_spawn_worker(&self, args, s, &worker, &channel);
  int spawn_error = errno;
  CHECK(spawned == (!strcmp(mode, "spawn-close-error") ? LGS_SPAWN_RETAINED
                                                       : LGS_SPAWN_READY));
  CHECK(worker > 0 && channel >= 0);
  CHECK(!close(sentinel));
  int control_fd = open(source, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
  CHECK(control_fd >= 0);
  struct lfg_message bind = context(s, t, worker, &self);
  struct control_native native = {mode, control_fd};
  struct lfgr_guardian guardian;
  CHECK(!lfgr_init(&guardian, &bind, worker, s, t, &control_ops, &native));
  if (spawned == LGS_SPAWN_RETAINED)
    lfgr_fail(&guardian, spawn_error, LFGR_S_CLOSE);
  svc.guardian = &guardian;
  svc.progress_fd = channel;
  svc.started = lg_milliseconds();
  svc.startup_ms = 300;
  svc.frame_ms = 100;
  svc.completion_ms = 250;
  svc.rpc_ms = 100;
  CHECK(!lgs_persist(&svc));
  CHECK(!lg_send(channel, &bind, lg_milliseconds() + 500));
  uint64_t end = lg_milliseconds() + 3000;
  for (;;) {
    lgs_step(&svc, 10);
    int marker = byte_at(control_fd);
    if (marker == 99 && guardian.wait_valid)
      break;
    if (lg_milliseconds() >= end) {
      if (!guardian.sticky_error)
        lfgr_fail(&guardian, ETIMEDOUT, LFGR_S_WORKER);
      if (guardian.wait_valid)
        break;
    }
  }
  int normal = guardian.retired && !guardian.sticky_error;
  int source_was_released = guardian.custody_fd < 0;
  /* Fault fixtures have no outer storage. This is explicit fixture recovery,
   * never a normal-release acknowledgement or production error-retire path. */
  if (guardian.custody_fd >= 0) {
    CHECK(!lg_barrier(guardian.custody_fd));
    CHECK(!close(guardian.custody_fd));
    guardian.custody_fd = -1;
  }
  CHECK(!lg_barrier(t));
  CHECK(!close(t));
  CHECK(!close(channel));
  CHECK(!close(self.fd));
  CHECK(!close(control_fd));
  CHECK(!lgs_socket_identity(&svc));
  CHECK(!unlinkat(svc.directory_fd, svc.socket_name, 0));
  CHECK(!close(svc.listener_fd));
  CHECK(!close(svc.directory_fd));
  char report[PATH_MAX];
  snprintf(report, sizeof report, "%s/result.json", dir);
  FILE *out = fopen(report, "wx");
  CHECK(out);
  fprintf(
      out,
      "{\"mode\":\"%s\",\"normal_completion\":%s,\"actual_worker_wait_kind\":%"
      "u,\"actual_worker_wait_value\":%u,\"sticky_error\":%d,\"source_released_"
      "by_runtime\":%s,\"fixture_recovery_cleanup\":%s,\"source_original_fd\":%"
      "d,\"status_guardian_fd\":%d,\"rpc_replies\":%u,\"rpc_send_errors\":%u,"
      "\"rpc_refusals\":%u,\"worker_timeouts\":%u}\n",
      mode, normal ? "true" : "false", guardian.current.wait_kind,
      guardian.current.wait_value, guardian.sticky_error,
      source_was_released ? "true" : "false", normal ? "false" : "true", s, t,
      svc.rpc_replies, svc.rpc_send_errors, svc.rpc_refusals,
      svc.worker_timeouts);
  CHECK(!fclose(out));
  return normal ? 0 : 1;
}
static int read_status(const char *dir, struct lfg_message *m) {
  char p[PATH_MAX];
  snprintf(p, sizeof p, "%s/status", dir);
  int fd = open(p, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
  if (fd < 0)
    return -1;
  uint8_t b[512];
  ssize_t n = pread(fd, b, sizeof b, 0);
  int saved = errno;
  close(fd);
  errno = saved;
  return n == 512 && lfg_decode(m, b, sizeof b) == 0 ? 0 : -1;
}
static void request_from(struct lfg_message *request,
                         const struct lfg_message *context, uint16_t kind) {
  *request = *context;
  request->kind = kind;
  request->state = LFG_REQUEST;
  request->flags = request->error = request->latched_error =
      request->error_stage = 0;
  request->completed_checks = 0;
  request->wait_kind = request->wait_value = 0;
  request->sequence = 0;
  CHECK(!getentropy(request->request_nonce, 16));
}
static int query(const char *dir, uint16_t kind, struct lfg_message *reply) {
  struct lfg_message m, request;
  char p[PATH_MAX];
  snprintf(p, sizeof p, "%s/s", dir);
  if (read_status(dir, &m))
    return -1;
  request_from(&request, &m, kind);
  return lg_call(p, &request, reply, lg_milliseconds() + 500);
}
static void await_state(const char *dir, int want_error, int want_clean,
                        struct lfg_message *reply) {
  uint64_t end = lg_milliseconds() + 1500;
  while (lg_milliseconds() < end) {
    if (!query(dir, LFG_QUERY, reply)) {
      if (want_error && reply->state == LFG_RETAINED)
        return;
      if (want_clean && lfg_clean_candidate(reply))
        return;
      if (!want_error && !want_clean &&
          (reply->state == LFG_RUNNING || reply->state == LFG_WORKER_DRAINED))
        return;
    }
    usleep(5000);
  }
  CHECK(0);
}
static int raw_connect(const char *dir) {
  char p[PATH_MAX];
  snprintf(p, sizeof p, "%s/s", dir);
  struct sockaddr_un a = {.sun_family = AF_UNIX};
  CHECK(strlen(p) < sizeof a.sun_path);
  strcpy(a.sun_path, p);
  a.sun_len = sizeof a;
  int fd = socket(AF_UNIX, SOCK_STREAM, 0);
  CHECK(fd >= 0);
  CHECK(!connect(fd, (struct sockaddr *)&a, sizeof a));
  return fd;
}
static void forge_status(const char *dir) {
  struct lfg_message m;
  CHECK(!read_status(dir, &m));
  m.kind = LFG_REPLY;
  m.state = LFG_RELEASED_CLEAN;
  m.flags = LFG_F_ALL;
  m.completed_checks = LFG_C_ALL;
  m.wait_kind = LFG_WAIT_EXITED;
  m.wait_value = 0;
  m.error = m.latched_error = m.error_stage = 0;
  ++m.sequence;
  uint8_t b[512];
  CHECK(!lfg_encode(b, sizeof b, &m));
  char p[PATH_MAX];
  snprintf(p, sizeof p, "%s/status", dir);
  int fd = open(p, O_RDWR | O_NOFOLLOW | O_CLOEXEC);
  CHECK(fd >= 0);
  CHECK(!lg_regular_fd(fd, getuid(), m.status_dev, m.status_ino, 0, 0));
  CHECK(pwrite(fd, b, sizeof b, 0) == sizeof b);
  CHECK(!fsync(fd));
  CHECK(!close(fd));
}
static void assert_claim(const char *dir) {
  char p[PATH_MAX];
  snprintf(p, sizeof p, "%s/source", dir);
  int fd = open(p, O_RDWR | O_NOFOLLOW | O_CLOEXEC);
  CHECK(fd >= 0);
  CHECK(flock(fd, LOCK_EX | LOCK_NB) < 0 &&
        (errno == EWOULDBLOCK || errno == EAGAIN));
  CHECK(!close(fd));
}
static void run_case(const char *self, const char *root, const char *mode) {
  char dir[PATH_MAX], log[PATH_MAX];
  CHECK(snprintf(dir, sizeof dir, "%s/%s", root, mode) < PATH_MAX);
  CHECK(!mkdir(dir, 0700));
  snprintf(log, sizeof log, "%s/service.log", dir);
  posix_spawn_file_actions_t a;
  posix_spawnattr_t attr;
  CHECK(!posix_spawn_file_actions_init(&a));
  CHECK(!posix_spawnattr_init(&attr));
  CHECK(!posix_spawnattr_setflags(&attr, POSIX_SPAWN_CLOEXEC_DEFAULT));
  CHECK(!posix_spawn_file_actions_addopen(&a, 0, "/dev/null", O_RDONLY, 0));
  CHECK(!posix_spawn_file_actions_addopen(&a, 1, log,
                                          O_WRONLY | O_CREAT | O_EXCL, 0600));
  CHECK(!posix_spawn_file_actions_adddup2(&a, 1, 2));
  char *argv[] = {(char *)self, "--service", dir, (char *)mode, NULL};
  pid_t daemon;
  CHECK(!posix_spawn(&daemon, self, &a, &attr, argv, environ));
  CHECK(!posix_spawn_file_actions_destroy(&a));
  CHECK(!posix_spawnattr_destroy(&attr));
  int error = !strcmp(mode, "startup-hang") || !strcmp(mode, "partial-hang") ||
              !strcmp(mode, "partial-eof") || !strcmp(mode, "worker-hang") ||
              !strcmp(mode, "forged-clean-exit1") ||
              !strcmp(mode, "spawn-close-error");
  struct lfg_message reply;
  if (!strcmp(mode, "startup-hang") || !strcmp(mode, "partial-hang") ||
      !strcmp(mode, "partial-eof") || !strcmp(mode, "spawn-close-error")) {
    await_state(dir, 1, 0, &reply);
    CHECK(!lfg_clean_candidate(&reply));
    assert_claim(dir);
    set_byte(dir, 1);
  } else {
    await_state(dir, 0, 0, &reply);
    assert_claim(dir);
    if (!strcmp(mode, "busy")) {
      CHECK(!query(dir, LFG_CLOSE, &reply));
      usleep(350000);
      CHECK(!query(dir, LFG_QUERY, &reply));
      CHECK(!reply.error && !lfg_clean_candidate(&reply));
    }
    if (!strcmp(mode, "partial-public")) {
      int fd = raw_connect(dir);
      CHECK(send(fd, "L", 1, 0) == 1);
      usleep(150000);
      CHECK(!close(fd));
      CHECK(!query(dir, LFG_QUERY, &reply) && !reply.error);
    }
    if (!strcmp(mode, "worker-hang")) {
      CHECK(!query(dir, LFG_CLOSE, &reply));
      await_state(dir, 1, 0, &reply);
      assert_claim(dir);
      set_byte(dir, 1);
    } else if (!strcmp(mode, "forged-clean-exit1")) {
      set_byte(dir, 1);
      uint64_t end = lg_milliseconds() + 1000;
      do {
        CHECK(!query(dir, LFG_QUERY, &reply));
        if (reply.state == LFG_WORKER_DRAINED)
          break;
        usleep(5000);
      } while (lg_milliseconds() < end);
      CHECK(reply.state == LFG_WORKER_DRAINED);
      forge_status(dir);
      set_byte(dir, 2);
      await_state(dir, 1, 0, &reply);
      struct lfg_message visible;
      CHECK(!read_status(dir, &visible));
      CHECK(lfg_clean_candidate(&visible));
      CHECK(!lfg_clean_candidate(&reply));
      assert_claim(dir);
    } else
      set_byte(dir, 1);
  }
  if (!error) {
    if (!strcmp(mode, "lost-reply")) {
      struct lfg_message m, request;
      uint64_t end = lg_milliseconds() + 1000;
      do {
        if (!read_status(dir, &m) && lfg_clean_candidate(&m))
          break;
        usleep(5000);
      } while (lg_milliseconds() < end);
      CHECK(lfg_clean_candidate(&m));
      for (int i = 0; i < 3; i++) {
        int fd = raw_connect(dir);
        request_from(&request, &m, LFG_QUERY);
        CHECK(!lg_send(fd, &request, lg_milliseconds() + 500));
        CHECK(!shutdown(fd, SHUT_RD));
        usleep(30000);
        CHECK(!close(fd));
      }
    }
    await_state(dir, 0, 1, &reply);
    CHECK(!query(dir, LFG_RETIRE, &reply));
    CHECK(reply.state == LFG_RETIRED);
  } else {
    await_state(dir, 1, 0, &reply);
    CHECK(!lfg_clean_candidate(&reply));
  }
  usleep(50000);
  set_byte(dir, 99);
  int status;
  CHECK(waitpid(daemon, &status, 0) == daemon);
  CHECK(WIFEXITED(status) && WEXITSTATUS(status) == (error ? 1 : 0));
  char report[PATH_MAX];
  snprintf(report, sizeof report, "%s/result.json", dir);
  FILE *f = fopen(report, "r");
  CHECK(f);
  char text[2048];
  size_t n = fread(text, 1, sizeof text - 1, f);
  text[n] = 0;
  CHECK(!fclose(f));
  CHECK(strstr(text, error ? "\"normal_completion\":false"
                           : "\"normal_completion\":true"));
  if (!strcmp(mode, "lost-reply"))
    CHECK(!strstr(text, "\"rpc_send_errors\":0"));
  if (!strcmp(mode, "partial-public"))
    CHECK(!strstr(text, "\"rpc_refusals\":0"));
  printf("%s", text);
}
int main(int argc, char **argv) {
  if (!getuid() || geteuid() != getuid() || getegid() != getgid())
    return 77;
  if (argc == 3 && !strcmp(argv[1], "--dummy"))
    return dummy_worker(argv[2]);
  if (argc == 4 && !strcmp(argv[1], "--service"))
    return service(argv[2], argv[3]);
  CHECK(argc == 2 && getuid() != 0 && getuid() == geteuid());
  struct stat st;
  CHECK(!lstat(argv[1], &st) && S_ISDIR(st.st_mode) && st.st_uid == getuid() &&
        !(st.st_mode & 0077));
  char self[PATH_MAX];
  CHECK(realpath(argv[0], self));
  const char *modes[] = {
      "normal",           "busy",         "lost-reply",
      "partial-public",   "startup-hang", "partial-hang",
      "partial-eof",      "worker-hang",  "forged-clean-exit1",
      "spawn-close-error"};
  for (size_t i = 0; i < sizeof modes / sizeof modes[0]; i++)
    run_case(self, argv[1], modes[i]);
  printf("{\"result\":\"pass\",\"checks\":%u,\"scope\":\"native guardian "
         "service plus dummy worker; no LKL/FUSE/mount/crypto/device\"}\n",
         checks);
  return 0;
}
