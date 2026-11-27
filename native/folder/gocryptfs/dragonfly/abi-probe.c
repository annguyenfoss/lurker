/* Compile/run on DragonFly only. Published stock wire header is supplied with
 * -I to the exact source tree; it is not presented as an installed header. */
#include <sys/param.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mount.h>
#include <sys/dirent.h>
#include <sys/syscall.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdio.h>
#include <vfs/fuse/fuse_abi.h>
static int first=1;
static void value(const char *name,unsigned long long n)
{ printf("%s\"%s\":%llu",first?"":",",name,n);first=0; }
#define SZ(t) value("sizeof." #t,sizeof(t))
#define OFF(t,f) value("offsetof." #t "." #f,offsetof(t,f))
#define VAL(n) value(#n,(unsigned long long)(n))
int main(void)
{
#ifndef __DragonFly__
#error native DragonFly compiler required
#endif
    putchar('{');SZ(struct dirent);OFF(struct dirent,d_ino);OFF(struct dirent,d_namlen);OFF(struct dirent,d_type);OFF(struct dirent,d_name);
    value("dirent.reclen.empty",_DIRENT_RECLEN(0));value("dirent.reclen.max",_DIRENT_RECLEN(255));
    SZ(struct stat);OFF(struct stat,st_ino);OFF(struct stat,st_atim);OFF(struct stat,st_mtim);OFF(struct stat,st_ctim);SZ(struct timespec);
    SZ(struct statfs);OFF(struct statfs,f_blocks);OFF(struct statfs,f_bavail);OFF(struct statfs,f_fsid);
    SZ(struct fuse_in_header);SZ(struct fuse_out_header);SZ(struct fuse_attr);SZ(struct fuse_entry_out);SZ(struct fuse_attr_out);SZ(struct fuse_open_out);SZ(struct fuse_kstatfs);SZ(struct fuse_init_in);SZ(struct fuse_init_out);SZ(struct fuse_dirent);
    OFF(struct fuse_attr,ino);OFF(struct fuse_attr,atimensec);OFF(struct fuse_attr,mode);OFF(struct fuse_attr,uid);OFF(struct fuse_attr,blksize);
    OFF(struct fuse_init_out,flags);OFF(struct fuse_init_out,max_write);OFF(struct fuse_init_out,time_gran);OFF(struct fuse_init_out,max_pages);OFF(struct fuse_init_out,unused);
    OFF(struct fuse_dirent,ino);OFF(struct fuse_dirent,off);OFF(struct fuse_dirent,namelen);OFF(struct fuse_dirent,type);OFF(struct fuse_dirent,name);
    VAL(FUSE_KERNEL_VERSION);VAL(FUSE_KERNEL_MINOR_VERSION);VAL(FUSE_MIN_READ_BUFFER);VAL(NAME_MAX);VAL(AT_SYMLINK_NOFOLLOW);VAL(UTIME_OMIT);VAL(SYS_futimens);VAL(SYS_utimensat);VAL(SYS_mknodat);VAL(O_NOFOLLOW);VAL(DT_REG);VAL(DT_DIR);
#ifdef O_PATH
    VAL(O_PATH);
#endif
#ifdef SYS_posix_fallocate
    VAL(SYS_posix_fallocate);
#endif
    puts("}");return 0;
}
