/* SPDX-License-Identifier: GPL-2.0-only */
/* Native host request evidence only; no LKL credentials are changed here. */
#ifndef LKLFUSE_NETBSD_CALLER_H
#define LKLFUSE_NETBSD_CALLER_H
#include <sys/types.h>
#include <sys/sysctl.h>
#include <sys/proc.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
struct netbsd_caller {
    int32_t pid;
    uint32_t uid, gid, ngroups;
    uint64_t birth_sec, birth_usec;
    uint32_t groups[KI_NGROUPS];
};
static int netbsd_snapshot(pid_t pid, uid_t uid, gid_t gid,
                           struct netbsd_caller *out)
{
    int mib[6] = {CTL_KERN, KERN_PROC2, KERN_PROC_PID, pid,
                  sizeof(struct kinfo_proc2), 1};
    struct kinfo_proc2 p;
    size_t bytes = sizeof(p);
    if (pid <= 0) return ESRCH;
    memset(&p, 0, sizeof(p));
    if (sysctl(mib, 6, &p, &bytes, NULL, 0) != 0) return errno;
    if (!bytes) return ESRCH;
    if (bytes != sizeof(p) || p.p_pid != pid) return EIO;
    if (p.p_stat == SZOMB || p.p_stat == SDEAD) return ESRCH;
    if (!p.p_uvalid || !p.p_ustart_sec || p.p_ustart_usec >= 1000000) return EIO;
    if (p.p_uid != uid || p.p_gid != gid) return EACCES;
    if (p.p_ngroups < 0 || p.p_ngroups > KI_NGROUPS) return EOVERFLOW;
    memset(out, 0, sizeof(*out));
    out->pid = pid; out->uid = uid; out->gid = gid;
    out->ngroups = p.p_ngroups;
    out->birth_sec = p.p_ustart_sec; out->birth_usec = p.p_ustart_usec;
    for (int i = 0; i < p.p_ngroups; i++) out->groups[i] = p.p_groups[i];
    return 0;
}
static int netbsd_stable_caller(pid_t pid, uid_t uid, gid_t gid,
                                struct netbsd_caller *out)
{
    struct netbsd_caller first, second;
    int error;
    for (int attempt = 0; attempt < 3; attempt++) {
        error = netbsd_snapshot(pid, uid, gid, &first);
        if (error) return error;
        error = netbsd_snapshot(pid, uid, gid, &second);
        if (error) return error;
        if (memcmp(&first, &second, sizeof(first)) == 0) {
            *out = first;
            return 0;
        }
    }
    return EAGAIN;
}
#endif
