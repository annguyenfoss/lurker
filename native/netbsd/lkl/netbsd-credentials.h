/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef LKLFUSE_NETBSD_CREDENTIALS_H
#define LKLFUSE_NETBSD_CREDENTIALS_H
#include "netbsd-caller.h"
#include <stdbool.h>
#include <lkl.h>

struct netbsd_credential {
    struct netbsd_caller caller;
    unsigned int umask;
};
/* The diagnostic frontend must enforce one callback thread. */
static bool netbsd_credentials_active;
static int netbsd_credential_poison;

static int netbsd_restore_credentials(void)
{
    long uid=lkl_sys_setresuid(0,0,0);
    long gid=lkl_sys_setresgid(0,0,0);
    long groups=lkl_sys_setgroups(0,NULL);
    long mask=lkl_sys_umask(0);
    bool failed=uid || gid || groups || mask<0;
    if (!failed)
        failed=lkl_sys_getuid()!=0 || lkl_sys_geteuid()!=0 ||
               lkl_sys_getgid()!=0 || lkl_sys_getegid()!=0 ||
               lkl_sys_getgroups(0,NULL)!=0 || lkl_sys_umask(0)!=0;
    if (failed) {
        if (!netbsd_credential_poison) netbsd_credential_poison=EIO;
        netbsd_credentials_active=true;
        /* Do not exit, close a channel or free source/LKL state here. */
        return EIO;
    }
    netbsd_credentials_active=false;
    return 0;
}

static int netbsd_enter_credentials(const struct netbsd_credential *credential,
                                    bool held_cleanup)
{
    const struct netbsd_caller *caller=&credential->caller;
    unsigned int groups[KI_NGROUPS];
    long ret;
    if ((credential->umask & ~0777U) || caller->ngroups>KI_NGROUPS)
        return EINVAL;
    if (netbsd_credential_poison && !held_cleanup) return EIO;
    if (netbsd_credentials_active) return EDEADLK;
    netbsd_credentials_active=true;
    for (unsigned int i=0;i<caller->ngroups;i++) groups[i]=caller->groups[i];
    ret=lkl_sys_setgroups(caller->ngroups,groups);
    if (!ret) ret=lkl_sys_setresgid(caller->gid,caller->gid,0);
    if (!ret) ret=lkl_sys_setresuid(caller->uid,caller->uid,0);
    if (!ret) ret=lkl_sys_umask(credential->umask);
    if (ret<0) {
        (void)netbsd_restore_credentials();
        return EIO;
    }
    return 0;
}

/* Only captured-authority barrier/release callbacks may use this after poison.
 * It never authenticates a new caller or permits namespace/metadata mutation. */
static int netbsd_enter_held_cleanup(const struct netbsd_credential *credential)
{
    if (netbsd_credentials_active && netbsd_restore_credentials()) return EIO;
    return netbsd_enter_credentials(credential,true);
}
#endif
