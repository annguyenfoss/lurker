/* SPDX-License-Identifier: GPL-2.0-only */
#define _NETBSD_SOURCE
#include "netbsd-mount.h"
#include <assert.h>
#include <stdio.h>
#include <unistd.h>
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);
    struct netbsd_mounts native={0};assert(!netbsd_mounts_get(&native) && native.count);
    printf("uid1300 native MNT_NOWAIT global inventory count=%zu; ",native.count);
    netbsd_mounts_free(&native);assert(!native.items && !native.count);
    struct statvfs old={.f_fsidx={{1,1}}},entries[3]={0};
    entries[0]=(struct statvfs){.f_fsidx={{2,2}},.f_owner=1300};
    strcpy(entries[0].f_fstypename,"puffs|lkl-unique");strcpy(entries[0].f_mntonname,"/private/mount");strcpy(entries[0].f_mntfromname,"/dev/puffs");
    struct netbsd_mounts before={.items=&old,.count=1},current={.items=entries,.count=1};
    struct netbsd_mount_identity identity={0};
    assert(netbsd_mount_present(&identity,&current)==-EINVAL);
    entries[0].f_owner=32767;assert(netbsd_mount_capture(&identity,&before,&current,"/private/mount","puffs|lkl-unique",1300)==ENOENT && !identity.captured);
    entries[0].f_owner=1300;entries[1]=entries[0];current.count=2;
    assert(netbsd_mount_capture(&identity,&before,&current,"/private/mount","puffs|lkl-unique",1300)==EEXIST && !identity.captured);
    current.count=1;old.f_fsidx=entries[0].f_fsidx;
    assert(netbsd_mount_capture(&identity,&before,&current,"/private/mount","puffs|lkl-unique",1300)==ESTALE && !identity.captured);
    old.f_fsidx=(fsid_t){{1,1}};
    entries[0].f_fsidx=(fsid_t){{0,0}};
    assert(netbsd_mount_capture(&identity,&before,&current,"/private/mount","puffs|lkl-unique",1300)==EIO && !identity.captured);
    entries[0].f_fsidx=(fsid_t){{2,2}};
    assert(!netbsd_mount_capture(&identity,&before,&current,"/private/mount","puffs|lkl-unique",1300));
    assert(netbsd_mount_present(&identity,&current)==1);
    assert(netbsd_mount_capture(&identity,&before,&current,"/private/mount","puffs|lkl-unique",1300)==EALREADY);
    strcpy(entries[0].f_mntonname,"/private/moved");entries[1]=entries[0];entries[1].f_fsidx=(fsid_t){{3,3}};strcpy(entries[1].f_mntonname,"/private/mount");current.count=2;
    assert(netbsd_mount_present(&identity,&current)==1);
    entries[0]=entries[1];current.count=1;assert(!netbsd_mount_present(&identity,&current));
    entries[0]=identity.mount;entries[0].f_owner=0;assert(netbsd_mount_present(&identity,&current)==-ESTALE);
    entries[0]=identity.mount;memset(entries[0].f_fstypename,'X',sizeof(entries[0].f_fstypename));assert(netbsd_mount_present(&identity,&current)==-ESTALE);
    puts("exact nonce/type/source/owner and full fsid capture rejects prior/duplicate identity; moved original remains present; replacement is not adopted; inconsistent/unterminated metadata fails closed; synthetic identity controls only, no native mount");
    return 0;
}
