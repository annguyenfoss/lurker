/* Native regression against the actual host virtio source, not a copy. */
#include <assert.h>
#include <stdlib.h>
#include <stdio.h>
#include LURKER_VIRTIO_SOURCE

struct lkl_host_operations lkl_host_ops = { .mem_alloc = malloc, .mem_free = free };
static int fail_request, fail_capacity, completed;
static int capacity(struct lkl_disk disk, unsigned long long *size)
{ (void)disk; *size = 512ULL * 1024 * 1024; return fail_capacity ? -LKL_EIO : 0; }
static int request(struct lkl_disk disk, struct lkl_blk_req *req)
{ (void)disk; (void)req; return fail_request ? LKL_DEV_BLK_STATUS_IOERR : LKL_DEV_BLK_STATUS_OK; }
struct lkl_dev_blk_ops lkl_dev_blk_ops = { .get_capacity = capacity, .request = request };
int virtio_dev_setup(struct virtio_dev *dev, int queues, int size)
{ static int id; dev->virtio_mmio_id = id++; (void)queues; (void)size; return 0; }
int virtio_dev_cleanup(struct virtio_dev *dev) { (void)dev; return 0; }
void virtio_req_complete(struct virtio_req *req, uint32_t len)
{ (void)req; (void)len; completed++; }
int lkl_printf(const char *format, ...) { (void)format; return 0; }

static int submit(struct lkl_disk disk, unsigned type, int malformed)
{
    struct lkl_virtio_blk_outhdr header = { .type = htole32(type) };
    struct virtio_blk_req_trailer status = { 99 };
    char data[16] = {0};
    struct virtio_req req = {0};
    req.buf[0].iov_base = &header; req.buf[0].iov_len = sizeof(header);
    req.buf_count = type == LKL_DEV_BLK_TYPE_FLUSH ? 2 : 3;
    if (req.buf_count == 3) { req.buf[1].iov_base = data; req.buf[1].iov_len = sizeof(data); }
    req.buf[req.buf_count-1].iov_base = &status;
    req.buf[req.buf_count-1].iov_len = sizeof(status);
    if (malformed) req.buf_count = 1;
    struct virtio_blk_dev *dev = disk.dev;
    int before = completed;
    assert(blk_enqueue(&dev->dev, 0, &req) == 0 && completed == before + 1);
    return status.status;
}
int main(void)
{
    struct lkl_disk first = {0}, second = {0};
    assert(lkl_disk_add(&first) >= 0 && lkl_disk_add(&second) >= 0);
    assert(lkl_disk_get_io_error(first) == 0 && lkl_disk_get_io_error(second) == 0);
    fail_request = 1;
    assert(submit(first, LKL_DEV_BLK_TYPE_FLUSH, 0) == LKL_DEV_BLK_STATUS_IOERR);
    fail_request = 0;
    assert(submit(first, LKL_DEV_BLK_TYPE_FLUSH, 0) == LKL_DEV_BLK_STATUS_OK);
    assert(lkl_disk_get_io_error(first) == -LKL_EIO);
    assert(lkl_disk_get_io_error(second) == 0);
    fail_request = 1;
    assert(submit(second, LKL_DEV_BLK_TYPE_WRITE, 0) == LKL_DEV_BLK_STATUS_IOERR);
    fail_request = 0;
    assert(submit(second, LKL_DEV_BLK_TYPE_WRITE, 0) == LKL_DEV_BLK_STATUS_OK);
    assert(lkl_disk_get_io_error(second) == -LKL_EIO);
    assert(lkl_disk_remove(first) == 0 && lkl_disk_remove(second) == 0);
    assert(lkl_disk_add(&first) >= 0 && lkl_disk_get_io_error(first) == 0);
    submit(first, LKL_DEV_BLK_TYPE_FLUSH, 1);
    assert(lkl_disk_get_io_error(first) == -LKL_EIO);
    assert(lkl_disk_remove(first) == 0);
    fail_capacity = 1;
    assert(lkl_disk_add(&first) < 0 && first.dev == NULL);
    assert(lkl_disk_get_io_error(first) == -LKL_EINVAL);
    puts("PASS: transient FLUSH/write errors sticky, disks isolated, fresh disk reset, malformed request and failed allocation safe");
    return 0;
}
