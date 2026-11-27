/* SPDX-License-Identifier: GPL-2.0-only */
/* Native diagnostic regular-file transport. No raw-device admission. */
#ifndef LKLFUSE_NETBSD_DISK_H
#define LKLFUSE_NETBSD_DISK_H
#include <sys/types.h>
#include <sys/stat.h>
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <unistd.h>
#include <lkl_host.h>

struct netbsd_disk {
    int fd;
    uint64_t bytes;
    dev_t device;
    ino_t inode;
    int first_error;
    unsigned long reads, writes, flushes;
};

static void netbsd_disk_error(struct netbsd_disk *disk, int error)
{
    int expected = 0;
    if (error <= 0) error = EIO;
    (void)__atomic_compare_exchange_n(&disk->first_error, &expected, error,
                                     0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST);
}

static int netbsd_disk_failed(struct netbsd_disk *disk)
{
    return __atomic_load_n(&disk->first_error, __ATOMIC_SEQ_CST);
}

/* Call after verified LKL drain/halt while the same source FD is still held. */
static int netbsd_disk_final_barrier(struct netbsd_disk *disk)
{
    if (fsync(disk->fd)) netbsd_disk_error(disk, errno);
    return netbsd_disk_failed(disk);
}

static int netbsd_disk_identify(struct netbsd_disk *disk, int fd)
{
    struct stat st;
    if (fstat(fd, &st)) return errno;
    if (!S_ISREG(st.st_mode) || st.st_uid != geteuid() || st.st_nlink != 1 ||
        (st.st_mode & 077) || st.st_size <= 0 || st.st_size % 512)
        return EINVAL;
    *disk = (struct netbsd_disk){.fd=fd, .bytes=st.st_size,
                               .device=st.st_dev, .inode=st.st_ino};
    return 0;
}

static int netbsd_disk_capacity(struct lkl_disk disk, unsigned long long *bytes)
{
    struct netbsd_disk *host = disk.handle;
    struct stat st;
    if (!host || fstat(host->fd, &st)) return -1;
    if (!S_ISREG(st.st_mode) || st.st_dev != host->device || st.st_ino != host->inode ||
        st.st_size < 0 || (uint64_t)st.st_size != host->bytes) {
        errno = EIO;
        netbsd_disk_error(host, EIO);
        return -1;
    }
    *bytes = host->bytes;
    return 0;
}

static int netbsd_disk_request(struct lkl_disk disk, struct lkl_blk_req *request)
{
    struct netbsd_disk *host = disk.handle;
    uint64_t offset, total = 0;
    int error = 0;
    if (!host || !request) return LKL_DEV_BLK_STATUS_IOERR;
    if (request->type == LKL_DEV_BLK_TYPE_FLUSH ||
        request->type == LKL_DEV_BLK_TYPE_FLUSH_OUT) {
        if (request->count != 0) error = EINVAL;
        else {
            __atomic_add_fetch(&host->flushes, 1, __ATOMIC_SEQ_CST);
            if (fsync(host->fd)) error = errno;
        }
        goto done;
    }
    if (request->type != LKL_DEV_BLK_TYPE_READ &&
        request->type != LKL_DEV_BLK_TYPE_WRITE)
        return LKL_DEV_BLK_STATUS_UNSUP;
    if (request->count <= 0 || request->count > 1024 || !request->buf ||
        request->sector > INT64_MAX / 512) {
        error = EINVAL;
        goto done;
    }
    offset = request->sector * 512;
    for (int i = 0; i < request->count; i++) {
        size_t size = request->buf[i].iov_len;
        if (size > INT64_MAX - total || (size && !request->buf[i].iov_base)) {
            error = EINVAL;
            goto done;
        }
        total += size;
    }
    if (offset > host->bytes || total > host->bytes - offset) {
        error = EIO;
        goto done;
    }
    if (request->type == LKL_DEV_BLK_TYPE_READ)
        __atomic_add_fetch(&host->reads, 1, __ATOMIC_SEQ_CST);
    else __atomic_add_fetch(&host->writes, 1, __ATOMIC_SEQ_CST);
    for (int i = 0; i < request->count; i++) {
        char *buffer = request->buf[i].iov_base;
        size_t size = request->buf[i].iov_len;
        while (size) {
            ssize_t amount;
            if (request->type == LKL_DEV_BLK_TYPE_READ)
                amount = pread(host->fd, buffer, size, offset);
            else amount = pwrite(host->fd, buffer, size, offset);
            if (amount < 0 && errno == EINTR) continue;
            if (amount <= 0) {
                error = amount < 0 ? errno : EIO;
                goto done;
            }
            buffer += amount;
            offset += amount;
            size -= amount;
        }
    }
done:
    if (error) {
        netbsd_disk_error(host, error);
        return LKL_DEV_BLK_STATUS_IOERR;
    }
    /* A later successful callback must never clear an earlier failure. */
    return LKL_DEV_BLK_STATUS_OK;
}

static struct lkl_dev_blk_ops netbsd_disk_ops = {
    .get_capacity=netbsd_disk_capacity, .request=netbsd_disk_request
};
#endif
