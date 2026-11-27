/* The native FUSE loop is single-threaded. Tokens identify registered handles,
 * never raw Linux descriptors/pointers. Kernel PID-zero cached I/O can use only
 * the validated opener identity associated with that exact held handle.
 */
#ifndef LKLFUSE_DRAGONFLY_HANDLES_H
#define LKLFUSE_DRAGONFLY_HANDLES_H
#include "dragonfly-credentials.h"

enum df_handle_kind { DF_FILE, DF_DIRECTORY };
struct df_handle {
    uint64_t token;
    uint64_t raw;
    int flags;
    enum df_handle_kind kind;
    struct df_request_identity opener;
    struct df_handle *next;
};
static struct df_handle *df_handles;
static int df_handle_error;
static uint64_t df_next_token = 1;

static struct df_handle *df_find_handle(uint64_t token, enum df_handle_kind kind)
{
    if (!token) return NULL;
    for (struct df_handle *h = df_handles; h; h = h->next)
        if (h->token == token && h->kind == kind) return h;
    return NULL;
}

static int df_enter_handle(const struct df_handle *handle, int allow_zero,
                           int cleanup)
{
    const struct fuse_context *context = fuse_get_context();
    if (cleanup || (context && context->pid == 0 && allow_zero))
        return df_apply_identity(&handle->opener);
    return df_enter_request(context);
}

static int df_open_handle(const char *path, mode_t mode,
                          struct fuse_file_info *fi, int create,
                          enum df_handle_kind kind)
{
    struct df_request_identity identity;
    int ret = df_validate_request(fuse_get_context(), &identity);
    if (ret < 0) return ret;
    if (!df_next_token) return -EOVERFLOW;
    struct df_handle *handle = calloc(1, sizeof(*handle));
    if (!handle) return -ENOMEM;
    ret = df_apply_identity(&identity);
    if (ret < 0) { free(handle); return ret; }
    struct fuse_file_info inner = *fi;
    inner.fh = 0;
    if (kind == DF_DIRECTORY) ret = lklfuse_opendir(path, &inner);
    else if (create) ret = lklfuse_create(path, mode, &inner);
    else ret = lklfuse_open(path, &inner);
    int restoration = df_leave_request();
    if (ret == 0 && restoration < 0) {
        /* The kernel never adopted this handle after a failed CREATE/OPEN.
         * Dispose only its exact held descriptor, with no pathname access. */
        int cleanup = kind == DF_DIRECTORY ?
            lkl_closedir((struct lkl_dir *)(uintptr_t)inner.fh) : lkl_sys_close(inner.fh);
        if (cleanup < 0) {
            df_handle_error = 1;
            fprintf(stderr, "lklfuse: unadopted handle cleanup failed: %d\n", cleanup);
        }
        free(handle);
        return -EIO;
    }
    if (!ret) {
        handle->token = df_next_token++;
        handle->raw = inner.fh;
        handle->flags = fi->flags;
        handle->kind = kind;
        handle->opener = identity;
        handle->next = df_handles;
        df_handles = handle;
        inner.fh = handle->token;
        *fi = inner;
    } else free(handle);
    if (restoration < 0) return -EIO;
    return ret;
}
static int df_open(const char *path, struct fuse_file_info *fi)
{ return df_open_handle(path, 0, fi, 0, DF_FILE); }
static int df_create(const char *path, mode_t mode, struct fuse_file_info *fi)
{ return df_open_handle(path, mode, fi, 1, DF_FILE); }
static int df_opendir(const char *path, struct fuse_file_info *fi)
{ return df_open_handle(path, 0, fi, 0, DF_DIRECTORY); }

#define DF_PATH(name, signature, arguments) \
static int df_##name signature { \
    int ret = df_enter_request(fuse_get_context()); \
    if (ret < 0) return ret; \
    ret = lklfuse_##name arguments; \
    return df_finish_request(ret); \
}
DF_PATH(getattr, (const char *path, struct stat *st), (path, st))
DF_PATH(readlink, (const char *path, char *buf, size_t size), (path, buf, size))
DF_PATH(mknod, (const char *path, mode_t mode, dev_t dev), (path, mode, dev))
DF_PATH(mkdir, (const char *path, mode_t mode), (path, mode))
DF_PATH(unlink, (const char *path), (path))
DF_PATH(rmdir, (const char *path), (path))
DF_PATH(symlink, (const char *oldname, const char *newname), (oldname, newname))
DF_PATH(rename, (const char *oldname, const char *newname), (oldname, newname))
DF_PATH(link, (const char *oldname, const char *newname), (oldname, newname))
DF_PATH(chmod, (const char *path, mode_t mode), (path, mode))
DF_PATH(chown, (const char *path, uid_t uid, gid_t gid), (path, uid, gid))
DF_PATH(truncate, (const char *path, off_t size), (path, size))
DF_PATH(statfs, (const char *path, struct statvfs *st), (path, st))
DF_PATH(access, (const char *path, int mode), (path, mode))
DF_PATH(utimens, (const char *path, const struct timespec tv[2]), (path, tv))
DF_PATH(setxattr, (const char *path, const char *name, const char *value, size_t size, int flags), (path, name, value, size, flags))
DF_PATH(getxattr, (const char *path, const char *name, char *value, size_t size), (path, name, value, size))
DF_PATH(listxattr, (const char *path, char *list, size_t size), (path, list, size))
DF_PATH(removexattr, (const char *path, const char *name), (path, name))
#undef DF_PATH

#define DF_HELD(name, signature, arguments, kind, zero, cleanup) \
static int df_##name signature { \
    struct df_handle *handle = df_find_handle(fi->fh, kind); \
    if (!handle) return -EBADF; \
    int ret = df_enter_handle(handle, zero, cleanup); \
    if (ret < 0) return ret; \
    struct fuse_file_info inner = *fi; \
    inner.fh = handle->raw; \
    ret = lklfuse_##name arguments; \
    return df_finish_request(ret); \
}
DF_HELD(fsync, (const char *path, int data, struct fuse_file_info *fi), (path, data, &inner), DF_FILE, 1, 0)
DF_HELD(readdir, (const char *path, void *buf, fuse_fill_dir_t fill, off_t offset, struct fuse_file_info *fi), (path, buf, fill, offset, &inner), DF_DIRECTORY, 1, 0)
DF_HELD(fsyncdir, (const char *path, int data, struct fuse_file_info *fi), (path, data, &inner), DF_DIRECTORY, 1, 0)
DF_HELD(fallocate, (const char *path, int mode, off_t offset, off_t length, struct fuse_file_info *fi), (path, mode, offset, length, &inner), DF_FILE, 0, 0)
#undef DF_HELD

static int df_flush(const char *path, struct fuse_file_info *fi)
{
    struct df_handle *h = df_find_handle(fi->fh, DF_FILE);
    if (!h) return -EBADF;
    /* The pinned upstream flush is a no-op, not a durability operation.
     * Cleanup uses only the registered capability and changes no credentials. */
    struct fuse_file_info inner = *fi; inner.fh = h->raw;
    return lklfuse_flush(path, &inner);
}

static int df_fgetattr(const char *path, struct stat *st, struct fuse_file_info *fi)
{
    struct df_handle *h = df_find_handle(fi->fh, DF_FILE);
    if (!h) h = df_find_handle(fi->fh, DF_DIRECTORY);
    if (!h) return -EBADF;
    int ret = df_enter_handle(h, 1, 0);
    if (ret < 0) return ret;
    struct fuse_file_info inner = *fi;
    inner.fh = h->kind == DF_DIRECTORY ?
        (uint64_t)lkl_dirfd((struct lkl_dir *)(uintptr_t)h->raw) : h->raw;
    ret = lklfuse_fgetattr(path, st, &inner);
    return df_finish_request(ret);
}

static int df_read(const char *path, char *buf, size_t size, off_t offset,
                    struct fuse_file_info *fi)
{
    struct df_handle *h = df_find_handle(fi->fh, DF_FILE);
    if (!h || (h->flags & O_ACCMODE) == O_WRONLY) return -EBADF;
    int ret = df_enter_handle(h, 1, 0);
    if (ret < 0) return ret;
    struct fuse_file_info inner = *fi; inner.fh = h->raw;
    ret = lklfuse_read(path, buf, size, offset, &inner);
    return df_finish_request(ret);
}
static int df_write(const char *path, const char *buf, size_t size, off_t offset,
                     struct fuse_file_info *fi)
{
    struct df_handle *h = df_find_handle(fi->fh, DF_FILE);
    if (!h || (h->flags & O_ACCMODE) == O_RDONLY) return -EBADF;
    int ret = df_enter_handle(h, 1, 0);
    if (ret < 0) return ret;
    struct fuse_file_info inner = *fi; inner.fh = h->raw;
    ret = lklfuse_write(path, buf, size, offset, &inner);
    return df_finish_request(ret);
}
static int df_close_handle(const char *path, struct fuse_file_info *fi,
                           enum df_handle_kind kind)
{
    struct df_handle *h = df_find_handle(fi->fh, kind);
    if (!h) return -EBADF;
    /* close/closedir use only this held open capability, never a pathname or
     * ambient-root authorization. They remain available after credential poison. */
    struct fuse_file_info inner = *fi; inner.fh = h->raw;
    int ret = kind == DF_DIRECTORY ? lklfuse_releasedir(path, &inner) :
                                    lklfuse_release(path, &inner);
    if (ret < 0) df_handle_error = 1;
    struct df_handle **link = &df_handles;
    while (*link != h) link = &(*link)->next;
    *link = h->next;
    memset(h, 0, sizeof(*h)); free(h); fi->fh = 0;
    return ret;
}

static int df_release(const char *path, struct fuse_file_info *fi)
{ return df_close_handle(path, fi, DF_FILE); }
static int df_releasedir(const char *path, struct fuse_file_info *fi)
{ return df_close_handle(path, fi, DF_DIRECTORY); }
#endif
