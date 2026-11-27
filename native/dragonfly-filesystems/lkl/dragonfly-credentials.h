/* Native process credentials for the single-threaded DragonFly FUSE loop.
 * Native sys/user.h and sys/sysctl.h must precede the Linux UAPI import.
 */
#ifndef LKLFUSE_DRAGONFLY_CREDENTIALS_H
#define LKLFUSE_DRAGONFLY_CREDENTIALS_H
#include "dragonfly-errno.h"

struct df_request_identity {
    pid_t pid;
    uid_t uid;
    int state;
    struct timeval birth;
    int count;
    gid_t groups[NGROUPS];
};

static int df_credential_snapshot(const struct fuse_context *context,
                                  struct df_request_identity *out)
{
    struct kinfo_proc process;
    size_t length = sizeof(process);
    if (!context || context->pid <= 0 || context->uid == (uid_t)-1 ||
        context->gid == (gid_t)-1)
        return -EACCES;
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, context->pid};
    if (sysctl(mib, 4, &process, &length, NULL, 0) != 0 ||
        length != sizeof(process) || process.kp_pid != context->pid ||
        process.kp_uid != context->uid ||
        (process.kp_stat != SACTIVE && process.kp_stat != SSTOP) ||
        process.kp_start.tv_sec <= 0 || process.kp_start.tv_usec < 0 ||
        process.kp_start.tv_usec >= 1000000 || process.kp_ngroups <= 0 ||
        process.kp_ngroups > NGROUPS || process.kp_groups[0] != context->gid)
        return -EACCES;
    out->pid = process.kp_pid;
    out->uid = process.kp_uid;
    out->state = process.kp_stat;
    out->birth = process.kp_start;
    out->count = process.kp_ngroups;
    for (int i = 0; i < out->count; i++) {
        if (process.kp_groups[i] == (gid_t)-1)
            return -EACCES;
        out->groups[i] = process.kp_groups[i];
    }
    return 0;
}

static int df_same_credentials(const struct df_request_identity *a,
                               const struct df_request_identity *b)
{
    return a->pid == b->pid && a->uid == b->uid && a->state == b->state &&
           a->birth.tv_sec == b->birth.tv_sec &&
           a->birth.tv_usec == b->birth.tv_usec && a->count == b->count &&
           !memcmp(a->groups, b->groups, a->count * sizeof(a->groups[0]));
}

/* Restore every field even after a partially applied request identity. */
static void df_leave_request(void)
{
    int uid = lkl_sys_setresuid(0, 0, 0);
    int groups = lkl_sys_setgroups(0, NULL);
    int gid = lkl_sys_setresgid(0, 0, 0);
    if (uid < 0 || groups < 0 || gid < 0) {
        fprintf(stderr, "lklfuse: cannot restore namespace credentials: %d/%d/%d\n",
                uid, groups, gid);
        _exit(1);
    }
}

static int df_enter_request(const struct fuse_context *context)
{
    struct df_request_identity first, second;
    if (df_credential_snapshot(context, &first) < 0 ||
        df_credential_snapshot(context, &second) < 0 ||
        !df_same_credentials(&first, &second))
        return -EACCES;
    lkl_gid_t groups[NGROUPS];
    for (int i = 0; i < second.count; i++)
        groups[i] = second.groups[i];
    long ret = lkl_sys_setgroups(0, NULL);
    if (!ret) ret = lkl_sys_setresgid(context->gid, context->gid, 0);
    if (!ret) ret = lkl_sys_setgroups(second.count, groups);
    if (!ret) ret = lkl_sys_setresuid(context->uid, context->uid, 0);
    if (ret < 0) {
        df_leave_request();
        return df_lkl_result(ret, 0);
    }
    return 0;
}
#endif
