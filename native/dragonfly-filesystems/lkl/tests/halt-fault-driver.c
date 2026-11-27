#include <sys/types.h>
#include <sys/mount.h>
#include <sys/file.h>
#include <sys/user.h>
#include <sys/sysctl.h>
#include <assert.h>
#include <signal.h>
#include <stdio.h>
#include <unistd.h>
#include <lkl_host.h>
static volatile sig_atomic_t halt_test_gate;
static int halt_test_failed;
static long halt_test_call(void);
#define lkl_sys_halt halt_test_call
#define main df_production_main
#include "../lklfuse.c"
#undef main
#undef lkl_sys_halt
static long halt_test_call(void)
{
    if(halt_test_gate && !halt_test_failed) {
        assert(lkl_sys_setresuid(1300,1300,0)==0);
        long ret=lkl_sys_halt();assert(ret==-LKL_EPERM && lkl_is_running());
        halt_test_failed=1;
        fprintf(stderr,"lklfuse-test: actual halt EPERM while kernel still running\n");
        return ret;
    }
    if(halt_test_failed && halt_test_gate) {
        struct fuse_chan *channel=fuse_session_next_chan(fuse_get_session(active_fuse),NULL);
        assert(channel && fcntl(fuse_chan_fd(channel),F_GETFD)>=0 && fcntl(lklfuse.disk.fd,F_GETFD)>=0 && lkl_is_running());
        fprintf(stderr,"lklfuse-test: retry gate retains source/channel after failed halt\n");
        while(halt_test_gate)usleep(10000);
    }
    long ret=lkl_sys_halt();
    fprintf(stderr,"lklfuse-test: actual later halt=%ld running=%d\n",ret,lkl_is_running());
    return ret;
}
static void toggle_halt_gate(int signal_number)
{(void)signal_number;halt_test_gate=!halt_test_gate;}
int main(int argc,char **argv)
{
    struct sigaction action={0};action.sa_handler=toggle_halt_gate;sigemptyset(&action.sa_mask);
    assert(sigaction(SIGUSR2,&action,NULL)==0);
    return df_production_main(argc,argv);
}
