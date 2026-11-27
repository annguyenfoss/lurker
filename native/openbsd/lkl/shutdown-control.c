/* SPDX-License-Identifier: GPL-2.0-only */
#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>
#include "openbsd-shutdown.h"
static int halt_calls,failures=3,barriers,closes,cleanups,barrier_error;
static long halt(void){halt_calls++;if(failures){failures--;return -1;}return 0;}
static int barrier(void *arg){(void)arg;barriers++;return barrier_error;}
static int channel(void *arg){int *fd=arg;closes++;return close(fd[0])?errno:0;}
static int source(void *arg){int *fd=arg;closes++;return close(fd[1])?errno:0;}
static void cleanup(void *arg){(void)arg;cleanups++;}
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);int fd[2];assert(!pipe(fd));
    struct ob_shutdown s={.published=true,.kernel_started=true,.channel_open=true,.source_open=true};
    struct ob_shutdown_ops ops={barrier,channel,source,cleanup};
    assert(ob_shutdown_halt(&s,halt)==EBUSY && ob_shutdown_release(&s,&ops,fd)==EBUSY);
    s.captured=true;assert(ob_shutdown_observe(&s,1)==EBUSY);assert(ob_shutdown_drain(&s,0)==EBUSY);
    assert(ob_shutdown_observe(&s,-1)==EIO && !s.absent);assert(!closes && !cleanups && !halt_calls);
    assert(!ob_shutdown_observe(&s,0));assert(ob_shutdown_halt(&s,halt)==EBUSY);
    assert(ob_shutdown_drain(&s,-1)==EIO && !s.drained);assert(!ob_shutdown_drain(&s,0));
    for(int i=0;i<3;i++){
        assert(ob_shutdown_halt(&s,halt)==EIO && !s.halted);assert(ob_shutdown_release(&s,&ops,fd)==EBUSY);
        assert(fcntl(fd[0],F_GETFD)>=0 && fcntl(fd[1],F_GETFD)>=0 && !closes && !barriers && !cleanups);
    }
    assert(!ob_shutdown_halt(&s,halt) && s.halted && halt_calls==4);
    barrier_error=ENOSPC;assert(ob_shutdown_release(&s,&ops,fd)==ENOSPC);
    assert(!closes && !cleanups && s.channel_open && s.source_open && fcntl(fd[1],F_GETFD)>=0);
    barrier_error=0;assert(ob_shutdown_release(&s,&ops,fd)==EIO && s.cleaned && closes==2 && cleanups==1);
    assert(ob_shutdown_release(&s,&ops,fd)==EIO && closes==2 && cleanups==1);
    puts("no-force release FSM retained actual pipe FDs through uncaptured/present/moved/query-error mounts, drain/three-halt/barrier failures; released once only after verified absence+drain+halt+barrier; sticky non-clean; no mount detector claim");
}
