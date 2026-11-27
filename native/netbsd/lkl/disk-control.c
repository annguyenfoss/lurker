/* SPDX-License-Identifier: GPL-2.0-only */
#define _NETBSD_SOURCE
#include <sys/types.h>
#include <sys/endian.h>
#include <sys/stat.h>
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

enum { NONE, READ_ONCE, WRITE_ONCE, FLUSH_ONCE };
static int fault;
static unsigned long native_flushes;
static ssize_t controlled_pread(int fd, void *buffer, size_t bytes, off_t offset)
{
    if (fault == READ_ONCE) { fault=NONE; errno=EIO; return -1; }
    return pread(fd,buffer,bytes,offset);
}
static ssize_t controlled_pwrite(int fd, const void *buffer, size_t bytes, off_t offset)
{
    if (fault == WRITE_ONCE) { fault=NONE; errno=EIO; return -1; }
    return pwrite(fd,buffer,bytes,offset);
}
static int controlled_fsync(int fd)
{
    native_flushes++;
    if (fault == FLUSH_ONCE) { fault=NONE; errno=EIO; return -1; }
    return fsync(fd);
}
#define pread controlled_pread
#define pwrite controlled_pwrite
#define fsync controlled_fsync
#include "netbsd-disk.h"
#undef pread
#undef pwrite
#undef fsync

int main(int argc,char **argv)
{
    assert(argc==2 && getuid()==1300 && geteuid()==1300);
    int fd=open(argv[1],O_RDWR|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);assert(fd>=0);
    printf("uid1300 native transport fixture=%s\n",argv[1]);fflush(stdout);
    assert(!ftruncate(fd,4096));
    struct netbsd_disk host;
    assert(!netbsd_disk_identify(&host,fd));
    struct lkl_disk disk={.handle=&host,.ops=&netbsd_disk_ops};
    unsigned long long capacity=0;assert(!disk.ops->get_capacity(disk,&capacity) && capacity==4096);
    char a[256],b[256],readback[512];memset(a,'a',sizeof(a));memset(b,'b',sizeof(b));
    struct iovec vectors[2]={{a,sizeof(a)},{b,sizeof(b)}};
    struct lkl_blk_req request={.type=LKL_DEV_BLK_TYPE_WRITE,.sector=1,.buf=vectors,.count=2};
    assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_OK);
    vectors[0]=(struct iovec){readback,sizeof(readback)};request.type=LKL_DEV_BLK_TYPE_READ;request.count=1;
    assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_OK);
    assert(!memcmp(a,readback,256) && !memcmp(b,readback+256,256));
    request=(struct lkl_blk_req){.type=LKL_DEV_BLK_TYPE_FLUSH,.count=0};
    assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_OK && native_flushes==1);
    assert(!netbsd_disk_final_barrier(&host));
    for(int which=READ_ONCE;which<=FLUSH_ONCE;which++) {
        assert(!netbsd_disk_identify(&host,fd));
        request=(struct lkl_blk_req){.type=which==READ_ONCE?LKL_DEV_BLK_TYPE_READ:which==WRITE_ONCE?LKL_DEV_BLK_TYPE_WRITE:LKL_DEV_BLK_TYPE_FLUSH,.sector=1,.buf=vectors,.count=which==FLUSH_ONCE?0:1};
        fault=which;assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_IOERR);
        assert(fault==NONE && netbsd_disk_failed(&host)==EIO);
        assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_OK);
        assert(netbsd_disk_final_barrier(&host)==EIO);
    }
    puts("actual native pread/pwrite/fsync boundary single-shot EIO injection stays sticky after later success and final barrier");
    assert(!netbsd_disk_identify(&host,fd));
    request=(struct lkl_blk_req){.type=LKL_DEV_BLK_TYPE_WRITE,.sector=UINT64_MAX,.buf=vectors,.count=1};
    assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_IOERR && netbsd_disk_failed(&host)==EINVAL);
    assert(!netbsd_disk_identify(&host,fd));request.sector=8;
    assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_IOERR && netbsd_disk_failed(&host)==EIO);
    assert(!netbsd_disk_identify(&host,fd));request.type=99;
    assert(disk.ops->request(disk,&request)==LKL_DEV_BLK_STATUS_UNSUP && !netbsd_disk_failed(&host));
    assert(!fchmod(fd,0644));assert(netbsd_disk_identify(&host,fd)==EINVAL);assert(!fchmod(fd,0600));
    char alias[PATH_MAX];assert(snprintf(alias,sizeof(alias),"%s.link",argv[1])>0);
    assert(!link(argv[1],alias));assert(netbsd_disk_identify(&host,fd)==EINVAL);assert(!unlink(alias));
    assert(!netbsd_disk_identify(&host,fd));assert(!ftruncate(fd,2048));
    assert(disk.ops->get_capacity(disk,&capacity)<0 && netbsd_disk_failed(&host)==EIO);
    assert(!close(fd));
    puts("regular-file/private-owner/size/identity/bounds/unsupported-op guards passed; no LKL boot or native mount");
    return 0;
}
