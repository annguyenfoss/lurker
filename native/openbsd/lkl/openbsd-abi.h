/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef LURKER_LKL_OPENBSD_ABI_H
#define LURKER_LKL_OPENBSD_ABI_H
#include <fcntl.h>
#include <limits.h>
#include <lkl.h>
#include <errno.h>
/* Reject unsupported flags; do not pass native bit numbers to Linux. */
static int ob_open_flags(int native, int *linux_flags)
{
    int flags=0, known=O_ACCMODE;
    switch(native&O_ACCMODE) {
    case O_RDONLY: flags=LKL_O_RDONLY;break;
    case O_WRONLY: flags=LKL_O_WRONLY;break;
    case O_RDWR: flags=LKL_O_RDWR;break;
    default:return EINVAL;
    }
#define OB_FLAG(name) do {known|=O_##name;if(native&O_##name)flags|=LKL_O_##name;} while(0)
    OB_FLAG(CREAT); OB_FLAG(EXCL); OB_FLAG(TRUNC); OB_FLAG(APPEND);
    OB_FLAG(NONBLOCK); OB_FLAG(SYNC); OB_FLAG(NOFOLLOW); OB_FLAG(DIRECTORY);
    OB_FLAG(CLOEXEC);
#undef OB_FLAG
    if(native&~known)return EOPNOTSUPP;
    *linux_flags=flags;return 0;
}
#endif
