/* Copy under tools/lkl/tests before compiling against that patched tree. */
#define main lklfuse_driver_main
#include "../lklfuse.c"
#undef main
#include <assert.h>
#include <stdatomic.h>

static int (*native_request)(struct lkl_disk, struct lkl_blk_req *);
static atomic_int fault, injected;
static int measured_request(struct lkl_disk disk, struct lkl_blk_req *req)
{
    if (atomic_load(&fault) && (req->type == LKL_DEV_BLK_TYPE_FLUSH ||
                               req->type == LKL_DEV_BLK_TYPE_FLUSH_OUT)) {
        disk.fd = -1;
        atomic_fetch_add(&injected, 1);
    }
    return native_request(disk, req);
}
int main(int argc, char **argv)
{
    assert(argc == 3 && getuid() == 1300 && geteuid() == 1300);
    int failing = !strcmp(argv[2], "fault");
    assert(failing || !strcmp(argv[2], "normal"));
    lklfuse.disk.fd = open(argv[1], O_RDWR);
    lklfuse.type = "xfs";
    lklfuse.mb = 128;
    assert(lklfuse.disk.fd >= 0);
    native_request = lkl_dev_blk_ops.request;
    lkl_dev_blk_ops.request = measured_request;
    lklfuse.disk_id = lkl_disk_add(&lklfuse.disk);
    assert(lklfuse.disk_id >= 0 && start_lkl() == 0);
    int fd = lkl_sys_open("/drain-control", LKL_O_CREAT | LKL_O_EXCL | LKL_O_WRONLY, 0600);
    assert(fd >= 0);
    assert(lkl_sys_write(fd, "native drain\n", 13) == 13);
    assert(lkl_sys_fsync(fd) == 0);
    assert(lkl_sys_write(fd, "dirty before drain\n", 19) == 19);
    atomic_store(&fault, failing);
    assert(lkl_sys_close(fd) == 0);
    int ret = stop_lkl();
    printf("mode=%s drain=%d injected_native_fsync_errors=%d uid=%lu euid=%lu\n",
           argv[2], ret, atomic_load(&injected), (unsigned long)getuid(), (unsigned long)geteuid());
    assert(failing ? (ret < 0 && atomic_load(&injected) > 0) : ret == 0);
    assert(close(lklfuse.disk.fd) == 0);
    return 0;
}
