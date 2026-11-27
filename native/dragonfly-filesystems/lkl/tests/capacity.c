#include <sys/stat.h>
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <lkl_host.h>
int main(int argc, char **argv)
{
    unsigned long long bytes;
    struct lkl_disk disk={0};
    if (argc==3 && !strcmp(argv[1],"--inspect")) {
        disk.fd=open(argv[2],O_RDONLY|O_NOFOLLOW|O_CLOEXEC);assert(disk.fd>=0);
        struct stat st;assert(fstat(disk.fd,&st)==0);
        assert(lkl_dev_blk_ops.get_capacity(disk,&bytes)==0);
        printf("held-fd capacity=%llu uid=%lu euid=%lu rdev=%llu ino=%llu readonly=1\n",bytes,(unsigned long)getuid(),(unsigned long)geteuid(),(unsigned long long)st.st_rdev,(unsigned long long)st.st_ino);
        assert(close(disk.fd)==0);return 0;
    }
    assert(argc==2 && getuid()==1300 && geteuid()==1300);
    disk.fd=open(argv[1],O_CREAT|O_EXCL|O_RDWR|O_CLOEXEC,0600);assert(disk.fd>=0);
    assert(ftruncate(disk.fd,524288)==0 && lkl_dev_blk_ops.get_capacity(disk,&bytes)==0 && bytes==524288);
    assert(ftruncate(disk.fd,513)==0);errno=0;
    assert(lkl_dev_blk_ops.get_capacity(disk,&bytes)<0 && errno==EINVAL);
    assert(lkl_disk_add(&disk)==-LKL_EIO);
    assert(close(disk.fd)==0);disk.fd=open("/dev/null",O_RDONLY|O_CLOEXEC);assert(disk.fd>=0);
    assert(lkl_dev_blk_ops.get_capacity(disk,&bytes)<0);assert(close(disk.fd)==0);
    disk.fd=-1;errno=0;assert(lkl_dev_blk_ops.get_capacity(disk,&bytes)<0 && errno==EBADF);
    puts("native regular512-unit capacity passed; irregular size, non-disk character node and invalid FD rejected; add-disk forwards I/O failure");return 0;
}
