/* SPDX-License-Identifier: MIT
 * Guardian/real-worker composition. Source/status are already exclusively
 * created/opened by the native owner; no file-only completion or public route.
 * Call before any LKL/FUSE/worker thread exists. No LKL operation occurs here.
 */
#ifndef LURKER_MACOS_LKL_GUARDIAN_LAUNCH_H
#define LURKER_MACOS_LKL_GUARDIAN_LAUNCH_H
#include "lkl-guardian-worker.h"
#include "lkl-guardian-native.h"
#include <sys/random.h>

struct lgd_session {
  struct lgs_executable executable;
  struct native_mount_watch mount;
  struct lg_native_binding binding;
  struct lfgr_guardian guardian;
  struct lgs_service service;
  char point[PATH_MAX];
  pid_t owned_child;
  int progress_fd, runtime_ready, launch_error;
};
/* Returns FAILED only with no child. RETAINED always exposes owned_child and
 * progress_fd: the owner must preserve source/status/executable/listener custody
 * and reap only that child. A partial launch can NEVER yield a positive RPC.
 * The new0600 single-link status FD is guardian-exclusive/CLOEXEC, never child5.
 * source_fd is the exact regular-file OFD inherited as child4; no pathname reopen.
 */
static inline int lgd_start(struct lgd_session *session, int source_fd,
                            int status_fd, const char *point, const char *type,
                            const char *private_directory) {
  if (!session || !point || !type || !private_directory) {
    errno = EINVAL;
    return LGS_SPAWN_FAILED;
  }
  memset(session, 0, sizeof(*session));
  session->progress_fd = -1;
  session->executable.fd = -1;
  session->service.listener_fd = session->service.directory_fd = -1;
  if (!getuid() || getuid() != geteuid() || getgid() != getegid() ||
      source_fd < 3 || status_fd < 3 || source_fd == status_fd ||
      !lgw_filesystem(type) ||
      fcntl(source_fd, F_SETFD, FD_CLOEXEC) ||
      fcntl(status_fd, F_SETFD, FD_CLOEXEC) ||
      !realpath(point, session->point) || lgs_self(&session->executable))
    return LGS_SPAWN_FAILED;
  session->mount.point = session->point;
  session->mount.require_writable = 1;
  if (lstat(session->point, &session->mount.underlay) ||
      !S_ISDIR(session->mount.underlay.st_mode) ||
      session->mount.underlay.st_uid != getuid() ||
      (session->mount.underlay.st_mode & 0077)) {
    errno = EPERM;
    return LGS_SPAWN_FAILED;
  }
  strcpy(session->mount.source, "native-lkl-");
  strcat(session->mount.source, type); /* whitelist above bounds this to16bytes */
  struct stat source, status;
  if (fstat(source_fd, &source) || fstat(status_fd, &status) ||
      source.st_size <= 0 ||
      (source.st_dev == status.st_dev && source.st_ino == status.st_ino) ||
      lg_regular_fd(source_fd, getuid(), source.st_dev, source.st_ino,
                    (uint64_t)source.st_size, 1) ||
      lg_regular_fd(status_fd, getuid(), status.st_dev, status.st_ino, 0, 0) ||
      flock(status_fd, LOCK_EX | LOCK_NB) ||
      lgs_listen(&session->service, private_directory, "s"))
    return LGS_SPAWN_FAILED;
  char *argv[] = {session->executable.path, "--worker-v2", session->point,
                  (char *)type, NULL};
  int spawned = lgs_spawn_worker(&session->executable, argv, source_fd,
                                 &session->owned_child, &session->progress_fd);
  if (spawned == LGS_SPAWN_FAILED)
    return spawned;
  session->launch_error = spawned == LGS_SPAWN_RETAINED ? errno : 0;
  struct lg_process owner, child;
  struct lfg_message m = {0};
  if (lg_process_snapshot(getpid(), &owner) ||
      lg_process_snapshot(session->owned_child, &child) ||
      lgw_boot(m.boot_id) || getentropy(m.session_nonce, 32) ||
      getentropy(m.request_nonce, 16)) {
    session->launch_error = errno ? errno : EIO;
    return LGS_SPAWN_RETAINED;
  }
  m.kind = LFG_BIND;
  m.state = LFG_STARTING;
  m.source_kind = LFG_SOURCE_REGULAR;
  m.filesystem = lgw_filesystem(type);
  m.required_checks = LFG_C_ALL;
  m.sequence = 1;
  m.uid = getuid();
  m.guardian_pid = getpid();
  m.guardian_birth_sec = owner.sec;
  m.guardian_birth_usec = owner.usec;
  m.worker_pid = session->owned_child;
  m.worker_birth_sec = child.sec;
  m.worker_birth_usec = child.usec;
  m.source_dev = source.st_dev;
  m.source_ino = source.st_ino;
  m.source_size = (uint64_t)source.st_size;
  m.status_dev = status.st_dev;
  m.status_ino = status.st_ino;
  m.native_id[2] = session->mount.underlay.st_dev;
  m.native_id[3] = session->mount.underlay.st_ino;
  memcpy(m.component_sha256, session->executable.sha256, 32);
  session->binding.context = &session->mount;
  session->binding.validate_ready = native_guardian_ready;
  session->binding.native_absent = native_guardian_absent;
  if (lg_runtime_init(&session->guardian, &m, session->owned_child, source_fd,
                       status_fd, &session->binding)) {
    session->launch_error = errno ? errno : EIO;
    return LGS_SPAWN_RETAINED;
  }
  session->runtime_ready = 1;
  session->service.guardian = &session->guardian;
  session->service.progress_fd = session->progress_fd;
  session->service.started = lg_milliseconds();
  /* Candidate bounds, subject to real startup/drain review before any pilot. */
  session->service.startup_ms = 30000;
  session->service.frame_ms = 5000;
  session->service.completion_ms = 120000;
  session->service.rpc_ms = 2000;
  if (session->launch_error)
    lfgr_fail(&session->guardian, session->launch_error, LFGR_S_CLOSE);
  if (lgs_persist(&session->service)) {
    session->launch_error = errno ? errno : EIO;
    lfgr_fail(&session->guardian, session->launch_error, LFGR_S_STATUS);
  }
  /* A known retained launch/status failure must not authorize writable setup.
   * With no BIND, the owned child's bounded receive expires without LKL init;
   * preserve channel/custody and observe its actual wait, never force it. */
  if (!session->launch_error &&
      lg_send(session->progress_fd, &m, lg_milliseconds() + 5000)) {
    session->launch_error = errno ? errno : EIO;
    lfgr_fail(&session->guardian, session->launch_error, LFGR_S_WORKER);
  }
  return session->launch_error ? LGS_SPAWN_RETAINED : LGS_SPAWN_READY;
}
static inline int lgd_step(struct lgd_session *session, unsigned wait_ms) {
  if (!session->runtime_ready) {
    errno = session->launch_error ? session->launch_error : EINVAL;
    return -1;
  }
  return lgs_step(&session->service, wait_ms);
}
/* RETIRED deliberately remains live/replayable. Production final service exit
 * and acknowledgement are unresolved; no shutdown/kill or protocol change is
 * invented here. The owner must not treat missing guardian as completion.
 */
#endif
