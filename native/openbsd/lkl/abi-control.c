/* SPDX-License-Identifier: GPL-2.0-only */
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <endian.h>
#include <netinet/in.h>
#include <signal.h>
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <unistd.h>
#define FUSE_USE_VERSION 26
#include <fuse/fuse.h>
#include "openbsd-errno.h"
#include "openbsd-abi.h"
#define OB_TYPE(field, signature) _Static_assert(__builtin_types_compatible_p(__typeof__(((struct fuse_operations*)0)->field),signature),"native API26 " #field)
OB_TYPE(getattr,int(*)(const char*,struct stat*));
OB_TYPE(fgetattr,int(*)(const char*,struct stat*,struct fuse_file_info*));
OB_TYPE(mknod,int(*)(const char*,mode_t,dev_t));
OB_TYPE(mkdir,int(*)(const char*,mode_t));
OB_TYPE(rename,int(*)(const char*,const char*));
OB_TYPE(chmod,int(*)(const char*,mode_t));
OB_TYPE(chown,int(*)(const char*,uid_t,gid_t));
OB_TYPE(truncate,int(*)(const char*,off_t));
OB_TYPE(open,int(*)(const char*,struct fuse_file_info*));
OB_TYPE(read,int(*)(const char*,char*,size_t,off_t,struct fuse_file_info*));
OB_TYPE(write,int(*)(const char*,const char*,size_t,off_t,struct fuse_file_info*));
OB_TYPE(statfs,int(*)(const char*,struct statvfs*));
OB_TYPE(flush,int(*)(const char*,struct fuse_file_info*));
OB_TYPE(release,int(*)(const char*,struct fuse_file_info*));
OB_TYPE(fsync,int(*)(const char*,int,struct fuse_file_info*));
OB_TYPE(readdir,int(*)(const char*,void*,fuse_fill_dir_t,off_t,struct fuse_file_info*));
OB_TYPE(utimens,int(*)(const char*,const struct timespec*));
#undef OB_TYPE
_Static_assert(sizeof(off_t)==8,"native file offsets");
_Static_assert(sizeof(((struct fuse_file_info*)0)->fh)==8,"native held token");
_Static_assert(S_IFREG==LKL_S_IFREG && S_IFDIR==LKL_S_IFDIR && S_IFLNK==LKL_S_IFLNK,"file type bridge");
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);
    assert(ob_lkl_result(-LKL_ENOTEMPTY,0)==-ENOTEMPTY);
    assert(ob_lkl_result(-LKL_EEXIST,0)==-EEXIST);
    assert(ob_lkl_result(-LKL_ENOSYS,0)==-ENOSYS);
    assert(ob_lkl_result(-LKL_EOPNOTSUPP,0)==-EOPNOTSUPP);
    assert(ob_lkl_result(-LKL_ENODATA,1)==-ENOATTR);
    assert(ob_lkl_result(-999999,0)==-EIO);
    assert(ob_lkl_result(LONG_MIN,0)==-EIO);
    assert(ob_lkl_result((long)INT_MAX+100,0)==(long)INT_MAX+100);
    assert(ob_fuse_result((long)INT_MAX+100,0)==-EOVERFLOW);
    assert(ob_fuse_result(INT_MAX,0)==INT_MAX);
    assert(ob_fuse_result(-LKL_ENOTEMPTY,0)==-ENOTEMPTY);
    assert(ob_lkl_result(0,0)==0);
    int flags;
    assert(!ob_open_flags(O_RDWR|O_CREAT|O_EXCL|O_CLOEXEC|O_SYNC,&flags));
    assert(flags==(LKL_O_RDWR|LKL_O_CREAT|LKL_O_EXCL|LKL_O_CLOEXEC|LKL_O_SYNC));
    assert(!ob_open_flags(O_RDONLY|O_NOFOLLOW|O_DIRECTORY,&flags));
    assert(flags==(LKL_O_RDONLY|LKL_O_NOFOLLOW|LKL_O_DIRECTORY));
    assert(ob_open_flags(O_ACCMODE,&flags)==EINVAL);
    assert(ob_open_flags(O_RDONLY|0x40000000,&flags)==EOPNOTSUPP);
    struct sigaction a={0};a.sa_handler=SIG_IGN;assert(a.sa_handler==SIG_IGN);
    siginfo_t info={0};info.si_pid=123;assert(info.si_pid==123);
    printf("native FUSE%u API26 field types/offsets/flags/signal aliases passed; Linux ENOSYS%d -> native%d, ENOTEMPTY%d -> native%d; unknown raw errors retained and fail EIO\n",FUSE_USE_VERSION,LKL_ENOSYS,ENOSYS,LKL_ENOTEMPTY,ENOTEMPTY);
}
