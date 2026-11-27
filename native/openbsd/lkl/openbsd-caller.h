/* SPDX-License-Identifier: GPL-2.0-only */
/* Live native request identity. FUSE pid is encoded THRID, never process PID. */
#ifndef LURKER_LKL_OPENBSD_CALLER_H
#define LURKER_LKL_OPENBSD_CALLER_H
#include <sys/types.h>
#include <sys/sysctl.h>
#include <signal.h>
#include <sys/proc.h>
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

struct ob_caller {
    int32_t pid, thrid;
    uint32_t uid, gid, ruid, rgid, svuid, svgid, ngroups;
    uint64_t birth_sec;
    uint32_t birth_usec, groups[KI_NGROUPS];
};

static int ob_caller_records(const void *records, size_t bytes, pid_t thrid,
                             uid_t uid, gid_t gid, struct ob_caller *out)
{
    const struct kinfo_proc *found = NULL, *rows = records;
    if (thrid <= 0 || uid == (uid_t)-1 || gid == (gid_t)-1) return ESRCH;
    if (!records || bytes % sizeof(*rows)) return EIO;
    for (size_t i = 0; i < bytes / sizeof(*rows); ++i) {
        if (rows[i].p_tid != thrid) continue;
        if (found) return EIO;
        found = &rows[i];
    }
    if (!found) return ESRCH;
    if (found->p_pid <= 0 || !found->p_uvalid || !found->p_ustart_sec ||
        found->p_ustart_usec >= 1000000) return ESRCH;
    if (found->p_stat != SRUN && found->p_stat != SSLEEP &&
        found->p_stat != SSTOP && found->p_stat != SONPROC) return ESRCH;
    if (found->p_uid != uid || found->p_gid != gid) return EACCES;
    if (found->p_ngroups <= 0 || found->p_ngroups > KI_NGROUPS) return EOVERFLOW;
    if (found->p_groups[0] != gid) return EACCES;
    memset(out, 0, sizeof(*out));
    out->pid = found->p_pid; out->thrid = thrid;
    out->uid = uid; out->gid = gid;
    out->ruid = found->p_ruid; out->rgid = found->p_rgid;
    out->svuid = found->p_svuid; out->svgid = found->p_svgid;
    out->birth_sec = found->p_ustart_sec; out->birth_usec = found->p_ustart_usec;
    out->ngroups = found->p_ngroups;
    for (unsigned i = 0; i < out->ngroups; ++i) {
        if (found->p_groups[i] == (gid_t)-1) return EACCES;
        out->groups[i] = found->p_groups[i];
    }
    return 0;
}

static int ob_snapshot(pid_t thrid, uid_t uid, gid_t gid, struct ob_caller *out)
{
    int mib[6] = {CTL_KERN, KERN_PROC, KERN_PROC_ALL | KERN_PROC_SHOW_THREADS,
                  0, sizeof(struct kinfo_proc), 0};
    for (unsigned attempt = 0; attempt < 3; ++attempt) {
        size_t bytes = 0;
        if (sysctl(mib, 6, NULL, &bytes, NULL, 0)) return errno;
        size_t count = bytes / sizeof(struct kinfo_proc) + 16;
        if (count > 65536) return EOVERFLOW;
        size_t capacity = count * sizeof(struct kinfo_proc);
        struct kinfo_proc *rows = calloc(count, sizeof(*rows));
        if (!rows) return ENOMEM;
        mib[5] = count; bytes = capacity;
        int result = sysctl(mib, 6, rows, &bytes, NULL, 0);
        int error = result ? errno : bytes > capacity ? EIO :
                    ob_caller_records(rows, bytes, thrid, uid, gid, out);
        free(rows);
        if (result && error == ENOMEM) continue;
        return error;
    }
    return EAGAIN;
}

static int ob_same_caller(const struct ob_caller *a, const struct ob_caller *b)
{ return memcmp(a, b, sizeof(*a)) == 0; }

typedef int (*ob_snapshot_fn)(pid_t, uid_t, gid_t, struct ob_caller *);
static int ob_stable_caller_with(ob_snapshot_fn snapshot, pid_t thrid,
                                 uid_t uid, gid_t gid, struct ob_caller *out)
{
    struct ob_caller first, second;
    int error = snapshot(thrid, uid, gid, &first);
    if (!error) error = snapshot(thrid, uid, gid, &second);
    if (error) return error;
    if (!ob_same_caller(&first, &second)) return EAGAIN;
    *out = second;
    return 0;
}
static int ob_stable_caller(pid_t thrid, uid_t uid, gid_t gid, struct ob_caller *out)
{ return ob_stable_caller_with(ob_snapshot, thrid, uid, gid, out); }
#endif
