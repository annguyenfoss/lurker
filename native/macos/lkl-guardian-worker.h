/* SPDX-License-Identifier: MIT
 * Private worker integration only. No public driver route or status-file writer.
 * Descriptor3 is inherited progress; descriptor4 is the guardian's exact source
 * open-file description. No source pathname or status descriptor is accepted.
 */
#ifndef LURKER_MACOS_LKL_GUARDIAN_WORKER_H
#define LURKER_MACOS_LKL_GUARDIAN_WORKER_H
#include "guardian-service.h"
#include <pthread.h>
#include <stdatomic.h>

struct lgw_worker {
  int active, ready, error_sent;
  atomic_int sticky_error;
  uint64_t completed;
  struct lfg_message identity;
  pthread_mutex_t lock;
};
static struct lgw_worker guarded_worker = {.lock = PTHREAD_MUTEX_INITIALIZER};
static inline void lgw_latch(int error) {
  int absent = 0;
  int value = error > 0 && error <= INT32_MAX ? error : EIO;
  atomic_compare_exchange_strong_explicit(&guarded_worker.sticky_error,
      &absent, value, memory_order_release, memory_order_relaxed);
}
/* Binding-owned tags; no new common wire or kernel ABI. */
static inline uint32_t lgw_filesystem(const char *type) {
  return !type ? 0u : !strcmp(type, "ext4") ? 1u : !strcmp(type, "xfs") ? 2u :
         !strcmp(type, "btrfs") ? 3u : 0u;
}
static inline int lgw_boot(unsigned char out[16]) {
  struct timeval boot;
  size_t length = sizeof(boot);
  int mib[] = {CTL_KERN, KERN_BOOTTIME};
  if (sysctl(mib, 2, &boot, &length, NULL, 0) || length != sizeof(boot) ||
      boot.tv_sec <= 0 || boot.tv_usec < 0 || boot.tv_usec >= 1000000) {
    errno = ESTALE;
    return -1;
  }
  lfg_put64(out, (uint64_t)boot.tv_sec);
  lfg_put64(out + 8, (uint64_t)boot.tv_usec);
  return 0;
}
/* First worker application operation, before LKL/FUSE/thread initialization.
 * A pre-spawn socketpair's native peer is its guardian creator. Exclusively
 * inherited descriptors + parent/worker birth + executable hash bind the child;
 * public accepted sockets use their own live-peer checks in guardian-service.h.
 */
static inline int lgw_bind(const char *type, const char *point) {
  if (!getuid() || geteuid() != getuid() || getegid() != getgid() ||
      fcntl(3, F_SETFD, FD_CLOEXEC) || fcntl(4, F_SETFD, FD_CLOEXEC))
    return -1;
  errno = 0;
  if (fcntl(5, F_GETFD) >= 0 || errno != EBADF) {
    errno = EPERM; /* production status is guardian-only, never descriptor5 */
    return -1;
  }
  struct stat channel, underlay;
  int kind = 0;
  socklen_t size = sizeof(kind);
  if (fstat(3, &channel) || !S_ISSOCK(channel.st_mode) ||
      getsockopt(3, SOL_SOCKET, SO_TYPE, &kind, &size) ||
      size != sizeof(kind) || kind != SOCK_STREAM || lstat(point, &underlay) ||
      !S_ISDIR(underlay.st_mode) || underlay.st_uid != getuid() ||
      (underlay.st_mode & 0077)) {
    errno = EPERM;
    return -1;
  }
  struct lgs_executable self;
  if (lgs_self(&self))
    return -1;
  struct lfg_message m;
  struct lg_process own, peer;
  unsigned char boot[16];
  int rc = lg_receive(3, &m, lg_milliseconds() + 5000);
  if (!rc && (lg_process_snapshot(getpid(), &own) || lg_peer(3, &peer) ||
              lgw_boot(boot)))
    rc = -1;
  if (!rc &&
      (m.kind != LFG_BIND || m.state != LFG_STARTING || m.sequence != 1 ||
       m.required_checks != LFG_C_ALL || m.completed_checks || m.flags ||
       m.error || m.latched_error || m.error_stage || m.wait_kind ||
       m.uid != getuid() || m.guardian_pid != (uint64_t)getppid() ||
       !lg_same_process(&own, m.worker_pid, m.worker_birth_sec,
                        m.worker_birth_usec) ||
       !lg_same_process(&peer, m.guardian_pid, m.guardian_birth_sec,
                        m.guardian_birth_usec) ||
       memcmp(boot, m.boot_id, 16) ||
       memcmp(self.sha256, m.component_sha256, 32) ||
       !lgw_filesystem(type) || m.filesystem != lgw_filesystem(type) ||
       m.native_id[0] || m.native_id[1] ||
       m.native_id[2] != (uint64_t)underlay.st_dev ||
       m.native_id[3] != (uint64_t)underlay.st_ino ||
       (fcntl(4, F_GETFL) & (O_ACCMODE | O_APPEND)) != O_RDWR ||
       lg_check_source(NULL, 4, &m) || flock(4, LOCK_EX | LOCK_NB))) {
    errno = ESTALE;
    rc = -1;
  }
  int error = errno;
  if (close(self.fd) && !rc) {
    rc = -1;
    error = errno;
  }
  if (rc) {
    errno = error ? error : EPROTO;
    return -1;
  }
  guarded_worker.identity = m;
  guarded_worker.active = 1;
  return 0;
}
/* All progress is serialized. Never call while holding the request/credential
 * lock. A failed send is permanent; the guardian must time out/observe exit1,
 * never infer completion from an absent worker or persisted file. */
static inline int lgw_send_locked(uint16_t kind, uint16_t state) {
  struct lgw_worker *w = &guarded_worker;
  if (w->identity.sequence == UINT64_MAX) {
    lgw_latch(EOVERFLOW);
    return -1;
  }
  struct lfg_message m = w->identity;
  m.kind = kind;
  m.state = state;
  m.sequence = ++w->identity.sequence;
  m.completed_checks = w->completed;
  if (kind == LFG_ERROR)
    m.error = m.latched_error = (uint32_t)w->sticky_error;
  if (lg_send(3, &m, lg_milliseconds() + 2000)) {
    lgw_latch(errno);
    return -1;
  }
  return 0;
}
static inline void lgw_fail(int error) {
  struct lgw_worker *w = &guarded_worker;
  if (!w->active)
    return;
  int e = pthread_mutex_lock(&w->lock);
  if (e) {
    /* A broken synchronization primitive cannot authorize DRAINED. */
    lgw_latch(e);
    return;
  }
  lgw_latch(error);
  if (!w->error_sent) {
    w->error_sent = 1;
    lgw_send_locked(LFG_ERROR, LFG_RETAINED);
  }
  if (pthread_mutex_unlock(&w->lock))
    lgw_latch(EIO);
}
static inline void lgw_complete(uint64_t checks) {
  if (!guarded_worker.active)
    return;
  /* Called by the main thread after observer join and completed native stages. */
  guarded_worker.completed |= checks & LFG_C_ALL;
}
static inline int lgw_ready(uint64_t first, uint64_t second) {
  struct lgw_worker *w = &guarded_worker;
  if (!w->active)
    return 0;
  int e = pthread_mutex_lock(&w->lock);
  if (e) {
    lgw_latch(e);
    return -1;
  }
  int rc = -1;
  if (!w->sticky_error && !w->ready && (first || second)) {
    w->identity.native_id[0] = first;
    w->identity.native_id[1] = second;
    rc = lgw_send_locked(LFG_READY, LFG_RUNNING);
    if (!rc)
      w->ready = 1;
  }
  e = pthread_mutex_unlock(&w->lock);
  if (e)
    lgw_latch(e);
  return rc;
}
static inline int lgw_finish(int exit_code) {
  struct lgw_worker *w = &guarded_worker;
  if (!w->active)
    return exit_code;
  if (exit_code || !w->ready || w->completed != LFG_C_ALL)
    lgw_fail(exit_code ? EIO : EPROTO);
  if (!w->sticky_error && lgw_send_locked(LFG_DRAINED, LFG_WORKER_DRAINED))
    exit_code = 1;
  if (close(3))
    exit_code = 1; /* actual owned wait0 remains mandatory at the guardian */
  return exit_code || w->sticky_error ? 1 : 0;
}
#endif
