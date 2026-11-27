/* Native FreeBSD FUSE3 ABI evidence and memory-only FFM controls.
 * No filesystem is mounted or opened by this executable/shared test library.
 */
#ifndef __FreeBSD__
#error This probe must be compiled on genuine FreeBSD
#endif
#define FUSE_USE_VERSION 31
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/param.h>
#include <sys/mount.h>
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <fuse.h>
#include <fuse_lowlevel.h>

typedef int (*native_getfsstat)(struct statfs *, long, int);
_Static_assert(__builtin_types_compatible_p(native_getfsstat,
               __typeof__(&getfsstat)), "getfsstat signature");

/* jFUSE calls the missing-extended-attribute error enodata(). FreeBSD uses
 * ENOATTR for this condition. This is a semantic native errno mapping. */
#ifndef ENODATA
#define ENODATA ENOATTR
#endif

#include "abi-fields.inc"

#ifndef ABI_SHARED
int main(void) {
    if (getuid() == 0 || geteuid() != getuid()) return 77;
    printf("{\"schema\":1,\"os\":\"FreeBSD\",\"architecture\":\"amd64\","
           "\"uid\":%u,\"euid\":%u,\"osreldate\":%d,\"fuse_version\":%d,",
           (unsigned)getuid(), (unsigned)geteuid(), __FreeBSD_version, fuse_version());
    printf("\"types\":{\"int\":%zu,\"long\":%zu,\"pointer\":%zu,"
           "\"mode_t\":%zu,\"size_t\":%zu,\"off_t\":%zu,"
           "\"uid_t\":%zu,\"gid_t\":%zu},",
           sizeof(int), sizeof(long), sizeof(void *), sizeof(mode_t),
           sizeof(size_t), sizeof(off_t), sizeof(uid_t), sizeof(gid_t));
    print_structs();
    print_constants();
    puts("}");
    return 0;
}
#endif
