/* SPDX-License-Identifier: MIT
 * Shared POSIX guardian state. No kernel, FUSE, mount, geometry, credentials,
 * transport, path lookup or process-disappearance authority. Native callbacks
 * supply measured host outcomes. Only owned waitpid supplies wait facts.
 */
#ifndef LURKER_LKL_GUARDIAN_RUNTIME_H
#define LURKER_LKL_GUARDIAN_RUNTIME_H
#include "guardian-protocol.h"
#include <errno.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

enum lfgr_result { LFGR_ERROR = -1, LFGR_OK = 0, LFGR_BUSY = 1 };
enum lfgr_stage {
  LFGR_S_WAIT = 1,
  LFGR_S_EXIT = 2,
  LFGR_S_WORKER = 3,
  LFGR_S_WORKER_ERROR = 4,
  LFGR_S_DESCRIPTOR = 5,
  LFGR_S_FLUSH = 6,
  LFGR_S_CLOSE = 7,
  LFGR_S_STATUS = 8,
  LFGR_S_NATIVE = 9,
  LFGR_S_READY = 10,
  LFGR_S_SEQUENCE = 11
};
/* Every callback is mandatory, native and serialized by the caller.
 * Callbacks must not re-enter this runtime or send public replies.
 * Return 0 on measured success, -1 with native errno on error. ONLY
 * native_absent can return 1 for retryable busy; other positive returns fail.
 * A callback must never map a native error a second time.
 * check_* validate held descriptor identity/ownership/mode and source geometry.
 * validate_ready binds the observed native mount instance to expected context.
 * native_absent repeats the captured mount identity/global absence check.
 * persist_status includes checked final barrier and writer-reference close,
 * retaining the guardian's separate status lock FD. It may leave valid-looking
 * bytes on failure; those bytes are NEVER authority.
 * close_source must set *fd=-1 once an actual close has been attempted (even
 * on error); do not retry an uncertain close or adopt/reopen a pathname. An
 * identity failure before attempting close may leave the held fd unchanged.
 */
struct lfgr_ops {
  int (*check_source)(void *, int, const struct lfg_message *);
  int (*check_status)(void *, int, const struct lfg_message *);
  int (*validate_ready)(void *, const struct lfg_message *,
                        const struct lfg_message *);
  int (*native_absent)(void *, const struct lfg_message *);
  int (*flush_source)(void *, int, const struct lfg_message *);
  int (*close_source)(void *, int *, const struct lfg_message *);
  int (*persist_status)(void *, int, const struct lfg_message *);
};
struct lfgr_guardian {
  struct lfg_message identity, current;
  const struct lfgr_ops *ops;
  void *native;
  pid_t child;
  int custody_fd, status_fd, wait_valid, wait_status;
  int sticky_error, error_stage, ready, drained, released, retired;
  uint64_t worker_sequence;
};
static inline int lfgr_native_error(int rc) {
  return rc == -1 && errno > 0 && errno <= INT32_MAX ? errno : EIO;
}
static inline void lfgr_fail(struct lfgr_guardian *g, int error, int stage) {
  if (!g->sticky_error) {
    g->sticky_error = error > 0 && error <= INT32_MAX ? error : EIO;
    g->error_stage = stage > 0 && stage <= 65535 ? stage : LFGR_S_WORKER;
  }
  g->current.kind = LFG_REPLY;
  g->current.state = LFG_RETAINED;
  g->current.error = g->current.latched_error = (uint32_t)g->sticky_error;
  g->current.error_stage = (uint32_t)g->error_stage;
  if (g->current.sequence != UINT64_MAX)
    ++g->current.sequence;
  errno = g->sticky_error;
}
static inline int lfgr_advance(struct lfgr_guardian *g) {
  if (g->current.sequence == UINT64_MAX) {
    lfgr_fail(g, EOVERFLOW, LFGR_S_SEQUENCE);
    return -1;
  }
  ++g->current.sequence;
  return 0;
}
/* Caller already spawned/owns this exact child, captured its birth, validated
 * all inherited descriptors and immutable boot/component/session identity, and
 * keeps the child blocked awaiting BIND. Failure transfers no ownership.
 * identity.request_nonce is a nonzero internal status-record nonce; each public
 * RPC still MUST carry its own fresh nonce and native peer authentication.
 */
static inline int lfgr_init(struct lfgr_guardian *g,
                            const struct lfg_message *identity, pid_t child,
                            int custody_fd, int status_fd,
                            const struct lfgr_ops *ops, void *native) {
  struct lfgr_guardian initial;
  if (!g || !identity || !ops || !ops->check_source || !ops->check_status ||
      !ops->validate_ready || !ops->native_absent || !ops->flush_source ||
      !ops->close_source || !ops->persist_status || child <= 0 ||
      (uint64_t)child != identity->worker_pid ||
      identity->guardian_pid != (uint64_t)getpid() || custody_fd < 0 ||
      status_fd < 0 || custody_fd == status_fd ||
      (identity->source_dev == identity->status_dev &&
       identity->source_ino == identity->status_ino) ||
      lfg_validate(identity) != LFG_OK || identity->kind != LFG_BIND ||
      identity->state != LFG_STARTING ||
      identity->required_checks != LFG_C_ALL || identity->completed_checks ||
      identity->flags || identity->wait_kind || identity->error ||
      identity->latched_error || identity->error_stage || !identity->sequence ||
      !lfg_nonzero(identity->request_nonce, 16)) {
    errno = EINVAL;
    return -1;
  }
  int rc = ops->check_source(native, custody_fd, identity);
  if (rc) {
    errno = lfgr_native_error(rc);
    return -1;
  }
  rc = ops->check_status(native, status_fd, identity);
  if (rc) {
    errno = lfgr_native_error(rc);
    return -1;
  }
  memset(&initial, 0, sizeof(initial));
  initial.identity = *identity;
  initial.current = *identity;
  initial.current.kind = LFG_REPLY;
  initial.child = child;
  initial.custody_fd = custody_fd;
  initial.status_fd = status_fd;
  initial.ops = ops;
  initial.native = native;
  initial.worker_sequence = identity->sequence;
  *g = initial;
  return 0;
}
/* Exact child ownership, not a same-OFD flock, pid existence probe or status
 * file, establishes termination. Call after draining available private worker
 * messages; READY received only after reap is conservatively rejected. */
static inline int lfgr_reap(struct lfgr_guardian *g, int nonblock) {
  int status;
  pid_t got;
  if (g->wait_valid)
    return 1;
  if (g->child <= 0 || (uint64_t)g->child != g->identity.worker_pid) {
    lfgr_fail(g, ECHILD, LFGR_S_WAIT);
    return -1;
  }
  do {
    got = waitpid(g->child, &status, nonblock ? WNOHANG : 0);
  } while (got < 0 && errno == EINTR);
  if (!got)
    return 0;
  if (got != g->child) {
    lfgr_fail(g, errno ? errno : ECHILD, LFGR_S_WAIT);
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
      lfgr_fail(g, EIO, LFGR_S_EXIT);
  } else {
    g->current.wait_kind = LFG_WAIT_SIGNALED;
    g->current.wait_value =
        WIFSIGNALED(status) ? (uint32_t)WTERMSIG(status) : 127;
    lfgr_fail(g, EINTR, LFGR_S_EXIT);
  }
  return 1;
}
/* Invoke only for frames from the verified private inherited worker channel.
 * Public client frames must never enter this function. Native callback checks
 * READY; thereafter native identity and all supervision fields are immutable.
 */
static inline int lfgr_worker_record(struct lfgr_guardian *g,
                                     const struct lfg_message *m) {
  if (lfg_validate(m) != LFG_OK || !lfg_same_binding(&g->identity, m) ||
      m->sequence <= g->worker_sequence || m->flags || m->wait_kind ||
      (g->ready &&
       memcmp(g->identity.native_id, m->native_id, sizeof(m->native_id)))) {
    lfgr_fail(g, ESTALE, LFGR_S_WORKER);
    return -1;
  }
  g->worker_sequence = m->sequence;
  if (m->kind == LFG_ERROR) {
    lfgr_fail(g, m->latched_error ? (int)m->latched_error : (int)m->error,
              LFGR_S_WORKER_ERROR);
    return -1;
  }
  if (g->sticky_error || g->released || g->retired) {
    lfgr_fail(g, g->sticky_error ? g->sticky_error : EPROTO, LFGR_S_WORKER);
    return -1;
  }
  if (m->kind == LFG_READY && !g->ready && !g->drained && !g->wait_valid &&
      !m->completed_checks &&
      (m->native_id[0] || m->native_id[1] || m->native_id[2] ||
       m->native_id[3])) {
    int rc = g->ops->validate_ready(g->native, &g->identity, m);
    if (rc) {
      lfgr_fail(g, lfgr_native_error(rc), LFGR_S_READY);
      return -1;
    }
    memcpy(g->identity.native_id, m->native_id, sizeof(m->native_id));
    memcpy(g->current.native_id, m->native_id, sizeof(m->native_id));
    g->ready = 1;
    g->current.state = LFG_RUNNING;
    return lfgr_advance(g);
  }
  if (m->kind == LFG_DRAINED && g->ready && !g->drained) {
    g->drained = 1;
    g->current.completed_checks = m->completed_checks;
    g->current.flags |= LFG_F_WORKER_DRAINED;
    g->current.state = LFG_WORKER_DRAINED;
    return lfgr_advance(g);
  }
  lfgr_fail(g, EPROTO, LFGR_S_WORKER);
  return -1;
}
/* Caller must report terminal private-channel EOF. Reaped0 without DRAINED
 * cannot stay indefinitely ambiguous or become file-only success. */
static inline int lfgr_channel_closed(struct lfgr_guardian *g) {
  if (!g->drained) {
    lfgr_fail(g, EPROTO, LFGR_S_WORKER);
    return -1;
  }
  return g->sticky_error ? -1 : 0;
}
static inline int lfgr_check_absent(struct lfgr_guardian *g) {
  int rc = g->ops->native_absent(g->native, &g->identity);
  if (!rc)
    return LFGR_OK;
  if (rc == LFGR_BUSY) {
    errno = EBUSY;
    return LFGR_BUSY;
  }
  lfgr_fail(g, lfgr_native_error(rc), LFGR_S_NATIVE);
  return LFGR_ERROR;
}
/* No source/backing release until owned child reaped0 and all worker stages.
 * A normal busy check is retryable. All native errors stay permanent, even if
 * a later operation succeeds. No callback can reset a recorded failure.
 */
static inline int lfgr_release(struct lfgr_guardian *g) {
  int rc;
  if (g->sticky_error) {
    errno = g->sticky_error;
    return LFGR_ERROR;
  }
  if (g->released)
    return LFGR_OK;
  if (!g->wait_valid || !WIFEXITED(g->wait_status) ||
      WEXITSTATUS(g->wait_status) || !g->drained || !g->ready ||
      g->current.completed_checks != LFG_C_ALL || g->custody_fd < 0) {
    errno = EBUSY;
    return LFGR_BUSY;
  }
  rc = lfgr_check_absent(g);
  if (rc)
    return rc;
  g->current.flags |= LFG_F_NATIVE_CLOSED;
  rc = g->ops->check_status(g->native, g->status_fd, &g->identity);
  if (!rc)
    rc = g->ops->check_source(g->native, g->custody_fd, &g->identity);
  if (rc) {
    lfgr_fail(g, lfgr_native_error(rc), LFGR_S_DESCRIPTOR);
    return LFGR_ERROR;
  }
  rc = g->ops->flush_source(g->native, g->custody_fd, &g->identity);
  if (rc) {
    lfgr_fail(g, lfgr_native_error(rc), LFGR_S_FLUSH);
    return LFGR_ERROR;
  }
  g->current.flags |= LFG_F_CUSTODY_FLUSHED;
  rc = g->ops->close_source(g->native, &g->custody_fd, &g->identity);
  if (rc || g->custody_fd != -1) {
    lfgr_fail(g, rc ? lfgr_native_error(rc) : EPROTO, LFGR_S_CLOSE);
    return LFGR_ERROR;
  }
  g->current.flags |= LFG_F_CUSTODY_CLOSED;
  g->current.kind = LFG_REPLY;
  g->current.state = LFG_RELEASED_CLEAN;
  g->current.flags |= LFG_F_STATUS_SYNCED;
  if (lfgr_advance(g))
    return LFGR_ERROR;
  /* Bytes may become visible before callback failure: not live authority. */
  rc = g->ops->persist_status(g->native, g->status_fd, &g->current);
  if (rc) {
    lfgr_fail(g, lfgr_native_error(rc), LFGR_S_STATUS);
    return LFGR_ERROR;
  }
  rc = lfgr_check_absent(g);
  if (rc) {
    /* Custody already closed; BUSY here is a changed final observation,
     * not retryable release. Never acknowledge the previously written file. */
    if (rc == LFGR_BUSY)
      lfgr_fail(g, EBUSY, LFGR_S_NATIVE);
    return LFGR_ERROR;
  }
  if (!lfg_clean_candidate(&g->current)) {
    lfgr_fail(g, EPROTO, LFGR_S_STATUS);
    return LFGR_ERROR;
  }
  g->released = 1;
  return LFGR_OK;
}
/* Prerequisites: native-authenticated current RPC peer and held session/socket
 * identity. This function does not authenticate a byte frame or status file.
 * The consumer must send RETIRE only AFTER confirmed outer-storage detach.
 * RETIRED stays live/replayable until the native service is explicitly closed.
 */
static inline int lfgr_reply(struct lfgr_guardian *g,
                             const struct lfg_message *request,
                             struct lfg_message *reply) {
  if (lfg_validate(request) != LFG_OK ||
      (request->kind != LFG_QUERY && request->kind != LFG_CLOSE &&
       request->kind != LFG_RETIRE) ||
      !lfg_same_binding(&g->identity, request) ||
      memcmp(g->identity.native_id, request->native_id,
             sizeof(request->native_id))) {
    errno = ESTALE;
    return -1;
  }
  /* Persist may expose complete bytes before failing. Even accidental
   * re-entrancy cannot publish that provisional in-memory state. */
  if (g->current.state == LFG_RELEASED_CLEAN && !g->released) {
    errno = EBUSY;
    return -1;
  }
  if (request->kind == LFG_RETIRE) {
    if (!g->released || g->sticky_error) {
      errno = EBUSY;
      return -1;
    }
    if (!g->retired) {
      if (lfgr_advance(g))
        return -1;
      g->retired = 1;
      g->current.state = LFG_RETIRED;
    }
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
  return lfg_validate(reply) == LFG_OK ? 0 : (errno = EPROTO, -1);
}
#endif
