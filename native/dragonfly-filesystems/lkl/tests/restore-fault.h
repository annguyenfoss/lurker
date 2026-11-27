/* Fault-test translation units only. No production option/environment activates
 * this shim. Keep the saved namespace UID0 so a later verified recovery is real. */
#include <sys/types.h>
#include <sys/mount.h>
#include <sys/file.h>
#include <sys/user.h>
#include <sys/sysctl.h>
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#define FUSE_USE_VERSION 26
#include <fuse.h>
#include <fuse_lowlevel.h>
#include <lkl_host.h>
static volatile sig_atomic_t df_test_restore_fault;
static unsigned df_test_restore_failures;
static long df_test_setresuid(lkl_uid_t real, lkl_uid_t effective, lkl_uid_t saved)
{
    if (df_test_restore_fault && real == 0 && effective == 0 && saved == 0 &&
        lkl_sys_geteuid() != 0) {
        /* UID65534 is neither current UID1300 nor saved UID0. The real Linux
         * syscall rejects this invalid restoration target with EPERM. */
        lkl_uid_t r, e, s;
        assert(lkl_sys_getresuid(&r, &e, &s) == 0);
        lkl_uid_t rejected = 65534;
        while (rejected == r || rejected == e || rejected == s) rejected--;
        long ret = lkl_sys_setresuid(rejected, 0, 0);
        assert(ret == -LKL_EPERM);
        df_test_restore_failures++;
        fprintf(stderr, "lklfuse-test: genuine restoration syscall EPERM count=%u\n", df_test_restore_failures);
        return ret;
    }
    return lkl_sys_setresuid(real, effective, saved);
}
#define lkl_sys_setresuid df_test_setresuid
