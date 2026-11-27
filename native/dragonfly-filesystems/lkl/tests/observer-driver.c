/* Standalone diagnostic client: hold the actual DESTROY callback before
 * its reply so the controller can inspect the kernel's unmounting FSID. */
#define FUSE_USE_VERSION 26
#include <fuse.h>
#include <signal.h>
#include <stdio.h>
#include <unistd.h>
#include <assert.h>
static volatile sig_atomic_t observer_test_gate=1;
static void observer_test_exit(struct fuse *filesystem);
#define fuse_exit observer_test_exit
#define main df_production_main
#include "../lklfuse.c"
#undef main
#undef fuse_exit
static void observer_test_exit(struct fuse *filesystem)
{
    /* Constructor cleanup before INIT is not an unmounting observation. */
    if(df_init_sent) {
        fprintf(stderr,"lklfuse-test: DESTROY reply held; native mount is unmounting\n");
        while(observer_test_gate)usleep(10000);
    }
    fuse_exit(filesystem);
}
static void release_observer(int number){(void)number;observer_test_gate=0;}
int main(int argc,char **argv)
{
    struct sigaction action={0};action.sa_handler=release_observer;sigemptyset(&action.sa_mask);
    assert(sigaction(SIGUSR2,&action,NULL)==0);
    return df_production_main(argc,argv);
}
