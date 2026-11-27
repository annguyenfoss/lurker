#include <assert.h>
#include <fcntl.h>
#include <stdatomic.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <lkl_host.h>

static int (*native_request)(struct lkl_disk, struct lkl_blk_req *);
static atomic_uint flushes, successful_flushes, failed_flushes;
static int fault;

static int measured_request(struct lkl_disk disk, struct lkl_blk_req *req)
{
    int flushing = req->type == LKL_DEV_BLK_TYPE_FLUSH ||
                   req->type == LKL_DEV_BLK_TYPE_FLUSH_OUT;
    if (flushing) {
        atomic_fetch_add(&flushes, 1);
        /* Use the genuine native backend's fsync error, not a synthetic status. */
        if (fault)
            disk.fd = -1;
    }
    int ret = native_request(disk, req);
    if (flushing)
        atomic_fetch_add(ret == LKL_DEV_BLK_STATUS_OK ?
                         &successful_flushes : &failed_flushes, 1);
    return ret;
}

int main(int argc, char **argv)
{
    assert(argc == 3 && getuid() == 1300 && geteuid() == 1300);
    assert(!strcmp(argv[2], "normal") || !strcmp(argv[2], "fault"));
    struct lkl_disk disk = { .fd = open(argv[1], O_RDWR) };
    assert(disk.fd >= 0);
    struct stat st;
    assert(!fstat(disk.fd, &st) && S_ISREG(st.st_mode));
    unsigned long long capacity;
    assert(lkl_dev_blk_ops.get_capacity(disk, &capacity) == 0);
    assert(capacity == (unsigned long long)st.st_size);
    native_request = lkl_dev_blk_ops.request;
    lkl_dev_blk_ops.request = measured_request;
    int id = lkl_disk_add(&disk);
    assert(id >= 0);
    assert(lkl_start_kernel(&lkl_host_ops, "mem=128M") == 0);
    char mount[64], path[128];
    assert(lkl_mount_dev(id, 0, "xfs", 0, NULL, mount, sizeof(mount)) == 0);
    snprintf(path, sizeof(path), "%s/flush-control", mount);
    int fd = lkl_sys_open(path, LKL_O_CREAT | LKL_O_EXCL | LKL_O_RDWR, 0600);
    assert(fd >= 0);
    const char payload[] = "native host flush control\n";
    assert(lkl_sys_write(fd, payload, sizeof(payload)) == sizeof(payload));
    assert(lkl_sys_fsync(fd) == 0);
    assert(atomic_load(&successful_flushes) > 0);
    unsigned int before = atomic_load(&flushes);
    fault = !strcmp(argv[2], "fault");
    assert(lkl_sys_write(fd, payload, sizeof(payload)) == sizeof(payload));
    long sync_ret = lkl_sys_fsync(fd);
    assert(atomic_load(&flushes) > before);
    if (fault) {
        assert(sync_ret == -LKL_EIO && atomic_load(&failed_flushes) > 0);
        fault = 0;
    } else {
        assert(sync_ret == 0 && atomic_load(&failed_flushes) == 0);
        char output[sizeof(payload)];
        assert(lkl_sys_pread64(fd, output, sizeof(output), 0) == sizeof(output));
        assert(!memcmp(payload, output, sizeof(payload)));
    }
    long close_ret = lkl_sys_close(fd);
    long unmount_ret = lkl_umount_dev(id, 0, 0, 10000);
    if (!strcmp(argv[2], "normal"))
        assert(close_ret == 0 && unmount_ret == 0);
    assert(lkl_sys_halt() == 0);
    assert(fsync(disk.fd) == 0 && close(disk.fd) == 0);
    printf("mode=%s capacity=%llu flushes=%u successful=%u failed=%u fsync=%ld close=%ld umount=%ld uid=%lu euid=%lu\n",
           argv[2], capacity, atomic_load(&flushes), atomic_load(&successful_flushes),
           atomic_load(&failed_flushes), sync_ret, close_ret, unmount_ret,
           (unsigned long)getuid(), (unsigned long)geteuid());
    return 0;
}
