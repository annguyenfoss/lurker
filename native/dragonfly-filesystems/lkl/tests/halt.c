#define main df_production_main
#include "../lklfuse.c"
#undef main
#include <assert.h>
#include <sys/wait.h>
int main(int argc,char **argv)
{
    assert(argc==2 && getuid()==1300 && geteuid()==1300);
    lklfuse.type="xfs";lklfuse.mb=128;
    lklfuse.disk.fd=open(argv[1],O_RDWR|O_NOFOLLOW|O_CLOEXEC);assert(lklfuse.disk.fd>=0);
    assert(flock(lklfuse.disk.fd,LOCK_EX|LOCK_NB)==0);
    lklfuse.disk_id=lkl_disk_add(&lklfuse.disk);assert(lklfuse.disk_id>=0 && start_lkl()==0);
    int fd=open("/dev/null",O_RDWR|O_CLOEXEC);assert(fd>=0);
    struct fuse_chan *channel=fuse_kern_chan_new(fd);assert(channel);
    char *name="native-halt-control";struct fuse_args args=FUSE_ARGS_INIT(1,&name);
    struct fuse_operations ops={0};struct fuse *fuse=fuse_new(channel,&args,&ops,sizeof(ops),NULL);assert(fuse);
    assert(drain_filesystem()==0);
    assert(lkl_sys_setresuid(1300,1300,0)==0);
    assert(df_halt_once()==-LKL_EPERM && df_halt_error && df_halt_pending && lkl_is_running());
    assert(fcntl(fd,F_GETFD)>=0 && fcntl(lklfuse.disk.fd,F_GETFD)>=0 && fuse_chan_session(channel)==fuse_get_session(fuse));
    pid_t child=fork();assert(child>=0);
    if(!child) {
        int other=open(argv[1],O_RDWR|O_NOFOLLOW|O_CLOEXEC);
        if(other<0)_exit(90);
        int ret=flock(other,LOCK_EX|LOCK_NB),error=errno;
        _exit(ret<0 && error==EWOULDBLOCK ? 0 : 91);
    }
    int status;assert(waitpid(child,&status,0)==child && WIFEXITED(status) && WEXITSTATUS(status)==0);
    assert(df_leave_request()==0);
    /* The same production wait gate sees actual halt0, but returns historical
     * failure. Only now is descriptor disposal permitted. */
    assert(df_wait_for_halt()==-LKL_EIO && !df_halt_pending && df_halt_error && !lkl_is_running());
    assert(fcntl(fd,F_GETFD)>=0 && fcntl(lklfuse.disk.fd,F_GETFD)>=0);
    fuse_opt_free_args(&args);fuse_destroy(fuse);assert(close(lklfuse.disk.fd)==0);
    puts("real halt EPERM retains running kernel, native channel and exclusive backing claim; verified recovery then halt0; sticky final failure retained");return 0;
}
