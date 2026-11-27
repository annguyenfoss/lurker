/* SPDX-License-Identifier: GPL-2.0-only */
/* Include the exact patched transport, with no kernel/device registration. */
#include <assert.h>
#include <endian.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <lkl_host.h>
#include "virtio_blk.c"
struct lkl_host_operations lkl_host_ops;
struct lkl_dev_blk_ops lkl_dev_blk_ops;
static unsigned calls,completions;static int status;
static uint64_t features;
int lkl_printf(const char *fmt,...) {(void)fmt;return 0;}
void virtio_req_complete(struct virtio_req *req,uint32_t len)
{(void)req;(void)len;completions++;}
int virtio_dev_setup(struct virtio_dev *dev,int queues,int maximum)
{assert(queues==1 && maximum==32);features=dev->device_features;dev->virtio_mmio_id=7;return 0;}
int virtio_dev_cleanup(struct virtio_dev *dev){(void)dev;return 0;}
static int capacity(struct lkl_disk disk,unsigned long long *bytes)
{(void)disk;*bytes=4096;return 0;}
static int request(struct lkl_disk disk,struct lkl_blk_req *req)
{(void)disk;calls++;assert(req->type==LKL_DEV_BLK_TYPE_FLUSH && req->count==0);return status;}
int main(void)
{
    lkl_host_ops.mem_alloc=malloc;lkl_host_ops.mem_free=free;
    struct lkl_dev_blk_ops ops={.get_capacity=capacity,.request=request};
    struct lkl_disk disk={.ops=&ops};
    assert(lkl_disk_add(&disk)==7 && features==0);assert(!lkl_disk_remove(disk));
    assert(lkl_disk_add_with_flush(&disk)==7 && features==(1ULL<<LKL_VIRTIO_BLK_F_FLUSH));
    struct virtio_blk_dev *dev=disk.dev;
    struct lkl_virtio_blk_outhdr header={.type=htole32(LKL_DEV_BLK_TYPE_FLUSH)};
    struct virtio_blk_req_trailer trailer={.status=255};
    struct virtio_req r={.buf_count=2};r.buf[0]=(struct iovec){&header,sizeof(header)};r.buf[1]=(struct iovec){&trailer,sizeof(trailer)};
    status=LKL_DEV_BLK_STATUS_OK;assert(!blk_enqueue(&dev->dev,0,&r));assert(calls==1 && completions==1 && trailer.status==status);
    status=LKL_DEV_BLK_STATUS_IOERR;assert(!blk_enqueue(&dev->dev,0,&r));assert(calls==2 && completions==2 && trailer.status==status);
    r.buf_count=3;r.buf[2]=r.buf[1];assert(!blk_enqueue(&dev->dev,0,&r));assert(calls==2 && trailer.status==LKL_DEV_BLK_STATUS_IOERR);
    r.buf_count=2;header.type=htole32(LKL_DEV_BLK_TYPE_READ);assert(!blk_enqueue(&dev->dev,0,&r));assert(calls==2 && trailer.status==LKL_DEV_BLK_STATUS_IOERR);
    header.type=htole32(LKL_DEV_BLK_TYPE_FLUSH);r.buf[0].iov_base=NULL;assert(!blk_enqueue(&dev->dev,0,&r) && calls==2);
    r.buf[0].iov_base=&header;r.buf[0].iov_len=1;assert(!blk_enqueue(&dev->dev,0,&r) && calls==2);
    r.buf[0].iov_len=sizeof(header);r.buf[1].iov_len=0;assert(!blk_enqueue(&dev->dev,0,&r) && calls==2);
    r.buf_count=VIRTIO_REQ_MAX_BUFS+1;assert(!blk_enqueue(&dev->dev,0,&r) && calls==2);
    assert(!lkl_disk_remove(disk));disk.ops=NULL;assert(lkl_disk_add_with_flush(&disk)==-LKL_EINVAL);
    puts("native exact transport codec: legacy feature0, custom opt-in FLUSH bit, 2-buffer zero-data request/status and EIO forwarding, malformed request rejection; no emulated disk or mount");
}
