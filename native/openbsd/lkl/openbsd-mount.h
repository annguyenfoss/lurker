/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef LURKER_LKL_OPENBSD_MOUNT_H
#define LURKER_LKL_OPENBSD_MOUNT_H
#include <sys/types.h>
#include <sys/mount.h>
#include <stdbool.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>

struct ob_mounts { struct statfs *items; size_t count; };
struct ob_mount_identity { struct statfs mount; bool captured; };

static inline void ob_mounts_free(struct ob_mounts *list)
{
    free(list->items);
    *list=(struct ob_mounts){0};
}

/* Serialize only identity fields, never native mount_info/padding pointers. */
static inline int ob_mount_sanitize(const struct statfs *input,struct statfs *output)
{
    if(!memchr(input->f_fstypename,0,sizeof(input->f_fstypename)) ||
       !memchr(input->f_mntfromname,0,sizeof(input->f_mntfromname)) ||
       !memchr(input->f_mntonname,0,sizeof(input->f_mntonname)))return EIO;
    struct statfs clean={0};clean.f_fsid=input->f_fsid;clean.f_owner=input->f_owner;
    clean.f_ctime=input->f_ctime;clean.f_flags=input->f_flags;
    strlcpy(clean.f_fstypename,input->f_fstypename,sizeof(clean.f_fstypename));
    strlcpy(clean.f_mntfromname,input->f_mntfromname,sizeof(clean.f_mntfromname));
    strlcpy(clean.f_mntonname,input->f_mntonname,sizeof(clean.f_mntonname));
    *output=clean;return 0;
}

/* Native host inventory, outside a host chroot. MNT_NOWAIT avoids stat RPCs,
 * but the kernel iterator may still wait for mount transactions. Run this in
 * a bounded native-only worker, never on the thread serving FUSE callbacks. */
static inline int ob_mounts_get(struct ob_mounts *list)
{
    for (int attempt=0;attempt<3;attempt++) {
        int count=getfsstat(NULL,0,MNT_NOWAIT);
        if (count<0) return errno;
        if (count>4088) return EOVERFLOW;
        size_t capacity=(size_t)count+8;
        struct statfs *items=calloc(capacity,sizeof(*items));
        if (!items) return ENOMEM;
        int actual=getfsstat(items,capacity*sizeof(*items),MNT_NOWAIT);
        if (actual<0) {int error=errno;free(items);return error;}
        if ((size_t)actual<capacity) {
            for(int i=0;i<actual;i++) {
                int error=ob_mount_sanitize(&items[i],&items[i]);
                if(error){free(items);return error;}
            }
            *list=(struct ob_mounts){.items=items,.count=actual};
            return 0;
        }
        free(items);
    }
    return EAGAIN;
}

static inline bool ob_mount_strings(const struct statfs *mount)
{
    return memchr(mount->f_fstypename,0,sizeof(mount->f_fstypename)) &&
           memchr(mount->f_mntfromname,0,sizeof(mount->f_mntfromname)) &&
           memchr(mount->f_mntonname,0,sizeof(mount->f_mntonname));
}

/* Observe only after a real callback was served, never merely from fuse init.
 * The native generic fusefs source is not a unique backing identity.
 * Bind this snapshot to the held backing/target/channel and a served nonce.
 * This operates on a worker's completed snapshot; it makes no kernel query. */
static inline int ob_mount_capture(struct ob_mount_identity *identity,
    const struct ob_mounts *before,const struct ob_mounts *current,
    const char *target,uid_t owner)
{
    const struct statfs *candidate=NULL;
    if (identity->captured) return EALREADY;
    for (size_t i=0;i<current->count;i++) {
        const struct statfs *entry=&current->items[i];
        if (!ob_mount_strings(entry)) return EIO;
        if (strcmp(entry->f_mntonname,target) || strcmp(entry->f_fstypename,"fuse") ||
            strcmp(entry->f_mntfromname,"fusefs") || entry->f_owner!=owner) continue;
        if (candidate) return EEXIST;
        candidate=entry;
    }
    if (!candidate) return ENOENT;
    fsid_t zero={0};
    if (!memcmp(&candidate->f_fsid,&zero,sizeof(zero))) return EIO;
    for (size_t i=0;i<before->count;i++)
        if (!memcmp(&before->items[i].f_fsid,&candidate->f_fsid,sizeof(fsid_t)))
            return ESTALE;
    identity->mount=*candidate;
    identity->captured=true;
    return 0;
}

/* 0: absent globally; 1: same mount still present, including at another path;
 * -errno: inconsistent/unknown. A replacement path never supplies identity. */
static inline int ob_mount_present(const struct ob_mount_identity *identity,
                                const struct ob_mounts *current)
{
    int found=0;
    if (!identity->captured) return -EINVAL;
    for (size_t i=0;i<current->count;i++) {
        const struct statfs *entry=&current->items[i];
        if (memcmp(&entry->f_fsid,&identity->mount.f_fsid,sizeof(fsid_t))) continue;
        if (++found>1 || !ob_mount_strings(entry) ||
            entry->f_owner!=identity->mount.f_owner ||
            strcmp(entry->f_fstypename,identity->mount.f_fstypename) ||
            strcmp(entry->f_mntfromname,identity->mount.f_mntfromname)) return -ESTALE;
    }
    return found;
}
#endif
