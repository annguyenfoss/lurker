/* SPDX-License-Identifier: GPL-2.0-only */
#define _NETBSD_SOURCE
#include "netbsd-observer.h"
#include <stdio.h>
static int write_all(const void *data,size_t bytes)
{
    const char *cursor=data;
    while(bytes) {
        ssize_t amount=write(STDOUT_FILENO,cursor,bytes);
        if(amount<0 && errno==EINTR)continue;
        if(amount<=0)return 1;
        cursor+=amount;bytes-=amount;
    }
    return 0;
}
int main(void)
{
    struct netbsd_mounts list={0};
    int error=netbsd_mounts_get(&list);
    if(error){errno=error;perror("native mount inventory");return 1;}
    struct netbsd_mount_wire header={NETBSD_MOUNT_WIRE_MAGIC,sizeof(struct statvfs),list.count};
    int failed=write_all(&header,sizeof(header)) || write_all(list.items,list.count*sizeof(*list.items));
    netbsd_mounts_free(&list);return failed;
}
