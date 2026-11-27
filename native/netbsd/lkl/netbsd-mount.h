/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef LKLFUSE_NETBSD_MOUNT_H
#define LKLFUSE_NETBSD_MOUNT_H
#include <sys/types.h>
#include <sys/mount.h>
#include <sys/statvfs.h>
#include <stdbool.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>

struct netbsd_mounts { struct statvfs *items; size_t count; };
struct netbsd_mount_identity { struct statvfs mount; bool captured; };

static void netbsd_mounts_free(struct netbsd_mounts *list)
{
    free(list->items);
    *list=(struct netbsd_mounts){0};
}

/* Native host inventory, outside a host chroot. MNT_NOWAIT avoids stat RPCs,
 * but the kernel iterator may still wait for mount transactions. Run this in
 * a bounded native-only worker, never on the thread serving PUFFS callbacks. */
static int netbsd_mounts_get(struct netbsd_mounts *list)
{
    for (int attempt=0;attempt<3;attempt++) {
        int count=getvfsstat(NULL,0,MNT_NOWAIT);
        if (count<0) return errno;
        if (count>4088) return EOVERFLOW;
        size_t capacity=(size_t)count+8;
        struct statvfs *items=calloc(capacity,sizeof(*items));
        if (!items) return ENOMEM;
        int actual=getvfsstat(items,capacity*sizeof(*items),MNT_NOWAIT);
        if (actual<0) {int error=errno;free(items);return error;}
        if ((size_t)actual<capacity) {
            *list=(struct netbsd_mounts){.items=items,.count=actual};
            return 0;
        }
        free(items);
    }
    return EAGAIN;
}

static bool netbsd_mount_strings(const struct statvfs *mount)
{
    return memchr(mount->f_fstypename,0,sizeof(mount->f_fstypename)) &&
           memchr(mount->f_mntfromname,0,sizeof(mount->f_mntfromname)) &&
           memchr(mount->f_mntonname,0,sizeof(mount->f_mntonname));
}

/* Observe only after a real callback was served, never merely from fuse init.
 * The frontend supplies a fresh short nonce in the exact puffs|type name.
 * This operates on a worker's completed snapshot; it makes no kernel query. */
static int netbsd_mount_capture(struct netbsd_mount_identity *identity,
    const struct netbsd_mounts *before,const struct netbsd_mounts *current,
    const char *target,const char *type,uid_t owner)
{
    const struct statvfs *candidate=NULL;
    if (identity->captured) return EALREADY;
    for (size_t i=0;i<current->count;i++) {
        const struct statvfs *entry=&current->items[i];
        if (!netbsd_mount_strings(entry)) return EIO;
        if (strcmp(entry->f_mntonname,target) || strcmp(entry->f_fstypename,type) ||
            strcmp(entry->f_mntfromname,"/dev/puffs") || entry->f_owner!=owner) continue;
        if (candidate) return EEXIST;
        candidate=entry;
    }
    if (!candidate) return ENOENT;
    fsid_t zero={0};
    if (!memcmp(&candidate->f_fsidx,&zero,sizeof(zero))) return EIO;
    for (size_t i=0;i<before->count;i++)
        if (!memcmp(&before->items[i].f_fsidx,&candidate->f_fsidx,sizeof(fsid_t)))
            return ESTALE;
    identity->mount=*candidate;
    identity->captured=true;
    return 0;
}

/* 0: absent globally; 1: same mount still present, including at another path;
 * -errno: inconsistent/unknown. A replacement path never supplies identity. */
static int netbsd_mount_present(const struct netbsd_mount_identity *identity,
                                const struct netbsd_mounts *current)
{
    int found=0;
    if (!identity->captured) return -EINVAL;
    for (size_t i=0;i<current->count;i++) {
        const struct statvfs *entry=&current->items[i];
        if (memcmp(&entry->f_fsidx,&identity->mount.f_fsidx,sizeof(fsid_t))) continue;
        if (++found>1 || !netbsd_mount_strings(entry) ||
            entry->f_owner!=identity->mount.f_owner ||
            strcmp(entry->f_fstypename,identity->mount.f_fstypename) ||
            strcmp(entry->f_mntfromname,identity->mount.f_mntfromname)) return -ESTALE;
    }
    return found;
}
#endif
