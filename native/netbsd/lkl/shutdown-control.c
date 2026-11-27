/* SPDX-License-Identifier: GPL-2.0-only */
#include "netbsd-shutdown.h"
#include <assert.h>
#include <stdio.h>
#include <unistd.h>
#include <fcntl.h>
#include <stdlib.h>
static int halt_calls, halt_failures, barriers, closes, cleanups, barrier_error;
static long halt(void) {halt_calls++;if(halt_failures){halt_failures--;return -5;}return 0;}
static int barrier(void *arg) {(void)arg;barriers++;return barrier_error;}
static int close_channel(void *arg) {int *fd=arg;closes++;return close(fd[0])?errno:0;}
static int close_source(void *arg) {int *fd=arg;closes++;return close(fd[1])?errno:0;}
static void cleanup(void *arg) {(void)arg;cleanups++;}
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);
    int fds[2];assert(!pipe(fds));
    struct netbsd_shutdown state={.published=true,.kernel_started=true,.channel_open=true,.source_open=true};
    struct netbsd_shutdown_ops ops={barrier,close_channel,close_source,cleanup};
    assert(netbsd_shutdown_halt(&state,halt)==EBUSY);
    assert(netbsd_shutdown_release(&state,&ops,fds)==EBUSY && !halt_calls && !closes && !cleanups);
    state.captured=true;
    /* Original mount moved and its path replaced: captured ID is still present. */
    assert(netbsd_shutdown_observe(&state,1)==EBUSY);
    assert(netbsd_shutdown_halt(&state,halt)==EBUSY && !halt_calls);
    assert(netbsd_shutdown_observe(&state,-1)==EIO && !state.absent);
    assert(netbsd_shutdown_release(&state,&ops,fds)==EBUSY && !closes);
    assert(!netbsd_shutdown_observe(&state,0));
    halt_failures=3;
    for(int i=0;i<3;i++) {
        assert(netbsd_shutdown_halt(&state,halt)==EIO && !state.halted);
        assert(netbsd_shutdown_release(&state,&ops,fds)==EBUSY);
        assert(fcntl(fds[0],F_GETFD)>=0 && fcntl(fds[1],F_GETFD)>=0);
        assert(!barriers && !closes && !cleanups);
    }
    assert(!netbsd_shutdown_halt(&state,halt) && state.halted && halt_calls==4);
    barrier_error=ENOSPC;
    assert(netbsd_shutdown_release(&state,&ops,fds)==EIO);
    assert(state.cleaned && !state.channel_open && !state.source_open && barriers==1 && closes==2 && cleanups==1);
    assert(netbsd_shutdown_release(&state,&ops,fds)==EIO && barriers==1 && closes==2 && cleanups==1);
    puts("uid1300 release gate retained actual descriptors across uncaptured/present/moved/query-error mount states and three negative halts; later halt0 released exactly once and retained failure; native FSID detection is a separate frontend gate");
    return 0;
}
