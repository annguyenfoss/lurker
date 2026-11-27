/* Native process credentials for the single-threaded DragonFly FUSE loop.
 * Native sys/user.h and sys/sysctl.h must precede the Linux UAPI import.
 */
#ifndef LKLFUSE_DRAGONFLY_CREDENTIALS_H
#define LKLFUSE_DRAGONFLY_CREDENTIALS_H
#include "dragonfly-errno.h"

static void (*df_observe_native_mount)(void);

struct df_request_identity {
    pid_t pid;
    uid_t uid;
    int state;
    struct timeval birth;
    int count;
    mode_t mask;
    gid_t groups[NGROUPS];
};

static int df_credential_snapshot(const struct fuse_context *context,
                                  struct df_request_identity *out)
{
    struct kinfo_proc process;
    size_t length = sizeof(process);
    if (!context || context->pid <= 0 || context->uid == (uid_t)-1 ||
        context->gid == (gid_t)-1 || (context->umask & ~0777))
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
    memset(out, 0, sizeof(*out));
    out->mask = context->umask & 0777;
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
           a->mask == b->mask &&
           !memcmp(a->groups, b->groups, a->count * sizeof(a->groups[0]));
}

/* A failed restoration poisons further work, but never exits a mounted
 * process. Only a verified complete restoration can clear current poison;
 * the historical failure remains sticky through final shutdown. */
static int df_credential_error, df_credentials_poisoned;
static int df_leave_request(void)
{
    int uid = lkl_sys_setresuid(0, 0, 0);
    int groups = lkl_sys_setgroups(0, NULL);
    int gid = lkl_sys_setresgid(0, 0, 0);
    int mask = lkl_sys_umask(0);
    lkl_uid_t ruid=(lkl_uid_t)-1, euid=ruid, suid=ruid;
    lkl_gid_t rgid=(lkl_gid_t)-1, egid=rgid, sgid=rgid;
    int got_uid = lkl_sys_getresuid(&ruid, &euid, &suid);
    int got_gid = lkl_sys_getresgid(&rgid, &egid, &sgid);
    int verified = uid == 0 && groups == 0 && gid == 0 && mask >= 0 &&
        got_uid == 0 && got_gid == 0 && !ruid && !euid && !suid &&
        !rgid && !egid && !sgid && lkl_sys_getgroups(0, NULL) == 0 &&
        lkl_sys_setfsuid((lkl_uid_t)-1) == 0 &&
        lkl_sys_setfsgid((lkl_gid_t)-1) == 0 && lkl_sys_umask(0) == 0;
    if (!verified) {
        if (!df_credentials_poisoned)
            fprintf(stderr, "lklfuse: poisoned credentials after restoration failure: %d/%d/%d/%d\n",
                    uid, groups, gid, mask);
        df_credential_error = df_credentials_poisoned = 1;
        return -EIO;
    }
    df_credentials_poisoned = 0;
    return 0;
}
static inline int df_finish_request(int result)
{
    return df_leave_request() < 0 ? -EIO : result;
}

static int df_validate_request(const struct fuse_context *context,
                               struct df_request_identity *identity)
{
    if (df_observe_native_mount) df_observe_native_mount();
    if (df_credentials_poisoned && df_leave_request() < 0) return -EIO;
    struct df_request_identity first, second;
    if (df_credential_snapshot(context, &first) < 0 ||
        df_credential_snapshot(context, &second) < 0 ||
        !df_same_credentials(&first, &second))
        return -EACCES;
    *identity = second;
    return 0;
}

static int df_apply_identity(const struct df_request_identity *identity)
{
    if (df_credentials_poisoned && df_leave_request() < 0) return -EIO;
    lkl_gid_t groups[NGROUPS];
    for (int i = 0; i < identity->count; i++)
        groups[i] = identity->groups[i];
    long ret = lkl_sys_setgroups(0, NULL);
    if (!ret) ret = lkl_sys_setresgid(identity->groups[0], identity->groups[0], 0);
    if (!ret) ret = lkl_sys_setgroups(identity->count, groups);
    if (!ret) ret = lkl_sys_setresuid(identity->uid, identity->uid, 0);
    if (!ret) {
        long mask = lkl_sys_umask(identity->mask);
        if (mask < 0) ret = mask;
    }
    if (ret < 0) {
        if (df_leave_request() < 0) return -EIO;
        return df_lkl_result(ret, 0);
    }
    return 0;
}

static int df_enter_request(const struct fuse_context *context)
{
    struct df_request_identity identity;
    int ret = df_validate_request(context, &identity);
    return ret < 0 ? ret : df_apply_identity(&identity);
}
#endif
