#include <sys/types.h>
#include <sys/param.h>
#include <sys/user.h>
#include <sys/sysctl.h>
#include <sys/wait.h>
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#define FUSE_USE_VERSION 26
#include <fuse.h>
#include <lkl_host.h>
#include "../dragonfly-credentials.h"

int main(void)
{
    assert(getuid() == 1300 && geteuid() == 1300);
    struct fuse_context context = {.pid = getpid(), .uid = geteuid(), .gid = getegid(), .umask = 0077};
    struct df_request_identity a, b;
    assert(df_credential_snapshot(&context, &a) == 0 &&
           df_credential_snapshot(&context, &b) == 0 && df_same_credentials(&a, &b));
    assert(lkl_start_kernel(&lkl_host_ops, "mem=128M") == 0);
    assert(df_enter_request(&context) == 0);
    assert(lkl_sys_umask(0077) == 0077);
    assert(lkl_sys_getuid() == 1300 && lkl_sys_geteuid() == 1300);
    assert(lkl_sys_getgid() == 1300 && lkl_sys_getegid() == 1300);
    lkl_gid_t groups[NGROUPS];
    assert(lkl_sys_getgroups(NGROUPS, groups) == a.count);
    /* Linux stores supplementary groups in sorted order; native BSD keeps
     * the effective primary group first. Compare the full group set. */
    for (int i = 0; i < a.count; i++) {
        int found = 0;
        for (int j = 0; j < a.count; j++) found += groups[j] == a.groups[i];
        assert(found == 1);
    }
    assert(lkl_sys_open("/missing/child", LKL_O_RDONLY, 0) == -LKL_ENOENT);
    assert(df_leave_request() == 0);
    assert(lkl_sys_getuid() == 0 && lkl_sys_geteuid() == 0 &&
           lkl_sys_getgid() == 0 && lkl_sys_getegid() == 0 &&
           lkl_sys_getgroups(0, NULL) == 0);
    assert(lkl_sys_umask(0) == 0);
    context.umask = 01000; assert(df_enter_request(&context) == -EACCES); context.umask = 0077;
    context.uid = 65534; assert(df_enter_request(&context) == -EACCES); context.uid = 1300;
    context.gid = 65534; assert(df_enter_request(&context) == -EACCES); context.gid = 1300;
    context.pid = 0; assert(df_enter_request(&context) == -EACCES);
    pid_t child = fork(); assert(child >= 0);
    if (child == 0) _exit(0);
    context.pid = child;
    /* This native kernel may omit zombies from KERN_PROC_PID. Prove the
     * owned child is exited but unreaped independently before the query. */
    siginfo_t exited;
    assert(waitid(P_PID, child, &exited, WEXITED | WNOWAIT) == 0);
    assert(df_enter_request(&context) == -EACCES);
    int status; assert(waitpid(child, &status, 0) == child && WIFEXITED(status));
    assert(df_enter_request(&context) == -EACCES);
    assert(lkl_sys_getuid() == 0 && lkl_sys_getgroups(0, NULL) == 0);
    assert(lkl_sys_halt() == 0);
    printf("native stable1048-byte process snapshots; actual uid/gid1300 and %d groups applied; syscall-error restore; mismatched uid/gid, pid0, zombie, reaped denied\n", a.count);
    return 0;
}
