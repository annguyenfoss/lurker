/* SPDX-License-Identifier: GPL-2.0-only
 * Real native mount identity shared by the worker and guardian callbacks.
 * getfsstat(MNT_NOWAIT) does not enter LKL or issue mounted-filesystem requests.
 * No mount/unmount, source lookup or emulated-kernel operation is performed.
 */
#ifndef LURKER_MACOS_LKL_GUARDIAN_NATIVE_H
#define LURKER_MACOS_LKL_GUARDIAN_NATIVE_H
#include "lkl-guardian.h"
#include <sys/mount.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
struct native_mount_watch {
  const char *point;
  char source[64];
  struct stat underlay;
  struct statfs captured;
  int captured_ok, error, done, require_writable;
};
static int native_mounts(struct statfs **out) {
  int n = getfsstat(NULL, 0, MNT_NOWAIT);
  if (n < 0)
    return -errno;
  if (n > 8192)
    return -EOVERFLOW;
  int cap = n + 16;
  struct statfs *v = calloc((size_t)cap, sizeof *v);
  if (!v)
    return -ENOMEM;
  n = getfsstat(v, cap * (int)sizeof *v, MNT_NOWAIT);
  if (n < 0) {
    int e = errno;
    free(v);
    return -e;
  }
  if (n >= cap) {
    free(v);
    return -EAGAIN;
  }
  *out = v;
  return n;
}
static int native_capture(struct native_mount_watch *w) {
  struct statfs *v = NULL;
  int n = native_mounts(&v);
  if (n < 0)
    return n;
  int rc = 0;
  for (int i = 0; i < n; i++)
    if (!strcmp(v[i].f_mntonname, w->point)) {
      if (strcmp(v[i].f_fstypename, "macfuse") ||
          strcmp(v[i].f_mntfromname, w->source) ||
          (w->require_writable && (v[i].f_flags & MNT_RDONLY))) {
        rc = -ESTALE;
        break;
      }
      if (!v[i].f_fsid.val[0] && !v[i].f_fsid.val[1]) {
        rc = -ESTALE;
        break;
      }
      w->captured = v[i];
      w->captured_ok = 1;
      fprintf(stderr, "NATIVE_MOUNT_ID fsid=%d:%d type=%s source=%s point=%s\n",
              v[i].f_fsid.val[0], v[i].f_fsid.val[1], v[i].f_fstypename,
              v[i].f_mntfromname, v[i].f_mntonname);
      break;
    }
  free(v);
  return rc;
}
static int native_mount_gone(const struct native_mount_watch *w) {
  if (w->error || !w->captured_ok)
    return w->error ? w->error : -ESTALE;
  struct statfs *v = NULL;
  int n = native_mounts(&v);
  if (n < 0)
    return n;
  int rc = 0;
  for (int i = 0; i < n; i++) {
    if (!memcmp(&v[i].f_fsid, &w->captured.f_fsid, sizeof(fsid_t))) {
      rc = -EBUSY;
      break;
    }
    if (!strcmp(v[i].f_mntonname, w->point)) {
      rc = -ESTALE;
      break;
    }
  }
  free(v);
  if (rc)
    return rc;
  int fd = open(w->point, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
  if (fd < 0)
    return -errno;
  struct stat st;
  rc = fstat(fd, &st);
  int e = errno;
  if (close(fd) && !rc) {
    rc = -1;
    e = errno;
  }
  if (rc)
    return -e;
  if (st.st_dev != w->underlay.st_dev || st.st_ino != w->underlay.st_ino ||
      st.st_uid != w->underlay.st_uid || (st.st_mode & 0022))
    return -ESTALE;
  return 0;
}

static inline int native_guardian_ready(void *context,
    const struct lfg_message *expected, const struct lfg_message *ready) {
  struct native_mount_watch *w = context;
  if (expected->native_id[2] != (uint64_t)w->underlay.st_dev ||
      expected->native_id[3] != (uint64_t)w->underlay.st_ino ||
      ready->native_id[2] != expected->native_id[2] ||
      ready->native_id[3] != expected->native_id[3]) {
    errno = ESTALE;
    return -1;
  }
  int rc = native_capture(w);
  if (rc || !w->captured_ok || !w->require_writable ||
      (w->captured.f_flags & MNT_RDONLY) ||
      ready->native_id[0] != (uint32_t)w->captured.f_fsid.val[0] ||
      ready->native_id[1] != (uint32_t)w->captured.f_fsid.val[1]) {
    errno = rc < 0 ? -rc : ESTALE;
    return -1;
  }
  return 0;
}
static inline int native_guardian_absent(void *context,
    const struct lfg_message *identity) {
  struct native_mount_watch *w = context;
  /* Before independently verified READY, absence is unknown, never clean. */
  if (!w->captured_ok)
    return LFGR_BUSY;
  if (identity->native_id[0] != (uint32_t)w->captured.f_fsid.val[0] ||
      identity->native_id[1] != (uint32_t)w->captured.f_fsid.val[1] ||
      identity->native_id[2] != (uint64_t)w->underlay.st_dev ||
      identity->native_id[3] != (uint64_t)w->underlay.st_ino) {
    errno = ESTALE;
    return -1;
  }
  int rc = native_mount_gone(w);
  if (!rc)
    return 0;
  if (rc == -EBUSY)
    return LFGR_BUSY;
  errno = -rc;
  return -1;
}
#endif
