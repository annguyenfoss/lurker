#include "restore-fault.h"
static struct fuse_context df_test_context;
static struct fuse_context *df_test_get_context(void) { return &df_test_context; }
#define fuse_get_context df_test_get_context
#define main df_production_main
#include "../lklfuse.c"
#undef main
#undef fuse_get_context
#undef lkl_sys_setresuid
int main(int argc, char **argv)
{
    assert(argc==2 && getuid()==1300 && geteuid()==1300);
    df_test_context=(struct fuse_context){.uid=1300,.gid=1300,.pid=getpid(),.umask=0077};
    lklfuse.type="xfs";lklfuse.mb=128;
    lklfuse.disk.fd=open(argv[1],O_RDWR|O_NOFOLLOW|O_CLOEXEC);assert(lklfuse.disk.fd>=0);
    assert(flock(lklfuse.disk.fd,LOCK_EX|LOCK_NB)==0);
    lklfuse.disk_id=lkl_disk_add(&lklfuse.disk);assert(lklfuse.disk_id>=0 && start_lkl()==0);
    struct fuse_file_info fi={.flags=O_CREAT|O_EXCL|O_RDWR};
    assert(df_create("/callback-held",0600,&fi)==0 && fi.fh!=0);
    uint64_t token=fi.fh;
    assert(df_write(NULL,"before fault\n",13,0,&fi)==13 && df_fsync(NULL,0,&fi)==0);
    struct stat st;assert(df_fgetattr(NULL,&st,&fi)==0 && st.st_uid==1300 && st.st_gid==1300 && (st.st_mode&0777)==0600);
    assert(df_chmod("/callback-held",06750)==0);
    df_test_context.pid=0;
    assert(df_mkdir("/pid0-denied",0700)==-EACCES);
    assert(df_write(NULL,"held pid0\n",10,13,&fi)==10 && df_fsync(NULL,0,&fi)==0);
    struct lkl_stat raw;assert(lkl_sys_lstat("/callback-held",&raw)==0 && !(raw.st_mode&06000));
    df_test_context.pid=getpid();
    df_test_restore_fault=1;
    assert(df_getattr("/callback-held",&st)==-EIO);
    assert(df_credential_error && df_credentials_poisoned && df_test_restore_failures>0);
    assert(df_mkdir("/poison-denied",0700)==-EIO);
    assert(lkl_sys_lstat("/poison-denied",&raw)==-LKL_ENOENT);
    df_test_context.pid=0;
    assert(df_write(NULL,"denied",6,0,&fi)==-EIO);
    assert(df_fsync(NULL,0,&fi)==-EIO);
    struct df_handle *held=df_find_handle(token,DF_FILE);assert(held);
    char unchanged[23];assert(lkl_sys_pread64(held->raw,unchanged,sizeof(unchanged),0)==23);
    assert(!memcmp(unchanged,"before fault\nheld pid0\n",23));
    assert(df_flush(NULL,&fi)==0);
    assert(df_release(NULL,&fi)==0 && !df_handles);
    fi.fh=token;assert(df_release(NULL,&fi)==-EBADF);
    assert(df_credentials_poisoned && df_credential_error);
    df_test_restore_fault=0;df_test_context.pid=getpid();
    assert(df_getattr("/callback-held",&st)==0);
    assert(!df_credentials_poisoned && df_credential_error);
    assert(lkl_sys_getuid()==0 && lkl_sys_geteuid()==0 && lkl_sys_getgroups(0,NULL)==0);
    assert(stop_lkl()==0 && close(lklfuse.disk.fd)==0);
    printf("actual callbacks uid/gid1300, PID0 held I/O, set-ID clearing, genuine restoration failures=%u, poison denies, registered cleanup, verified recovery and sticky failure passed\n",df_test_restore_failures);
    return 0;
}
