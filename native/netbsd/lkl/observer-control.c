/* SPDX-License-Identifier: GPL-2.0-only */
#define _NETBSD_SOURCE
#include <time.h>
#include <errno.h>
static int clock_failure;
static int controlled_clock(clockid_t id,struct timespec *now)
{ if(clock_failure){errno=EIO;return -1;}return clock_gettime(id,now); }
#define clock_gettime controlled_clock
#include "netbsd-observer.h"
#undef clock_gettime
#include <assert.h>
#include <stdio.h>
extern char **environ;
static void finish(struct netbsd_observer *job)
{
    uint64_t end=netbsd_observer_now()+4000000000ULL;
    while(job->active && netbsd_observer_now()<end){netbsd_observer_tick(job);usleep(1000);}
    assert(!job->active);
}
int main(int argc,char **argv)
{
    if(argc==2 && !strcmp(argv[1],"--ignore-term")) {
        assert(signal(SIGTERM,SIG_IGN)!=SIG_ERR);
        assert(write(STDOUT_FILENO,"READY",5)==5);
        for(;;)pause();
    }
    if(argc==2 && !strcmp(argv[1],"--invalid")) {
        assert(write(STDOUT_FILENO,"wrong-protocol",14)==14);return 0;
    }
    assert(argc==3 && getuid()==1300 && geteuid()==1300);
    struct netbsd_observer job={0};struct netbsd_mounts result={0};
    char *normal[]={argv[1],NULL};assert(!netbsd_observer_start(&job,normal,environ,2000));
    assert(getpgid(job.pid)==job.pid);
    assert(netbsd_observer_start(&job,normal,environ,2000)==EBUSY);
    finish(&job);assert(!netbsd_observer_take(&job,&result) && result.count);
    printf("uid1300 isolated native inventory worker returned %zu native records; ",result.count);
    netbsd_mounts_free(&result);
    char *invalid[]={argv[2],"--invalid",NULL};assert(!netbsd_observer_start(&job,invalid,environ,2000));
    finish(&job);assert(netbsd_observer_take(&job,&result)==EIO && !result.items);
    char *ignore[]={argv[2],"--ignore-term",NULL};assert(!netbsd_observer_start(&job,ignore,environ,1000));
    pid_t pid=job.pid;int fd=job.fd;
    uint64_t ready=netbsd_observer_now()+500000000ULL;
    while(job.bytes<5 && netbsd_observer_now()<ready){netbsd_observer_tick(&job);usleep(1000);}
    assert(job.bytes==5 && !memcmp(job.buffer,"READY",5));
    finish(&job);assert(job.stopping && job.killed && !job.stuck && job.error==ETIMEDOUT);
    assert(netbsd_observer_take(&job,&result)==ETIMEDOUT && !result.items);
    int status;assert(waitpid(pid,&status,WNOHANG)==-1 && errno==ECHILD);
    assert(fcntl(fd,F_GETFD)==-1 && errno==EBADF);
    assert(!netbsd_observer_start(&job,ignore,environ,2000));
    ready=netbsd_observer_now()+500000000ULL;
    while(job.bytes<5 && netbsd_observer_now()<ready){netbsd_observer_tick(&job);usleep(1000);}
    assert(job.bytes==5 && !memcmp(job.buffer,"READY",5));
    clock_failure=1;finish(&job);clock_failure=0;
    assert(job.killed && job.error==EIO && !job.result.items);
    puts("native process-group deadline killed/reaped TERM-ignoring worker after grace; clock failure cancels immediately; only owned IPC closes; malformed/timeout snapshots rejected; no LKL syscall or native mount");
    return 0;
}
