/* SPDX-License-Identifier: GPL-2.0-only */
#include <time.h>
#include <errno.h>
static int clock_failure;
static int controlled_clock(clockid_t id,struct timespec *now)
{ if(clock_failure){errno=EIO;return -1;}return clock_gettime(id,now); }
#define clock_gettime controlled_clock
#include "openbsd-observer.h"
#undef clock_gettime
#include <assert.h>
#include <stdio.h>
extern char **environ;
static void finish(struct ob_observer *job)
{
    uint64_t end=ob_observer_now()+4000000000ULL;
    while(job->active && ob_observer_now()<end){ob_observer_tick(job);usleep(1000);}
    assert(!job->active);
}
static void identities(void)
{
    struct statfs old={0},current[2]={{0}};
    old.f_fsid=(fsid_t){{1,2}};
    current[0].f_fsid=(fsid_t){{5,6}};current[0].f_owner=0;
    strlcpy(current[0].f_fstypename,"fuse",sizeof(current[0].f_fstypename));
    strlcpy(current[0].f_mntfromname,"fusefs",sizeof(current[0].f_mntfromname));
    strlcpy(current[0].f_mntonname,"/not-mounted/owned",sizeof(current[0].f_mntonname));
    memset(&current[0].mount_info,0xa5,sizeof(current[0].mount_info));
    struct statfs sanitized;
    assert(!ob_mount_sanitize(&current[0],&sanitized));
    unsigned char zero[sizeof(sanitized.mount_info)]={0};
    assert(!memcmp(&sanitized.mount_info,zero,sizeof(zero)));
    struct ob_mounts before={&old,1},now={current,1};struct ob_mount_identity identity={0};
    assert(!ob_mount_capture(&identity,&before,&now,"/not-mounted/owned",0));
    assert(ob_mount_present(&identity,&now)==1);
    strlcpy(current[0].f_mntonname,"/not-mounted/moved",sizeof(current[0].f_mntonname));
    assert(ob_mount_present(&identity,&now)==1);
    strlcpy(current[0].f_fstypename,"ffs",sizeof(current[0].f_fstypename));
    assert(ob_mount_present(&identity,&now)==-ESTALE);
    strlcpy(current[0].f_fstypename,"fuse",sizeof(current[0].f_fstypename));
    current[1]=current[0];now.count=2;assert(ob_mount_present(&identity,&now)==-ESTALE);
    now.count=0;assert(ob_mount_present(&identity,&now)==0);
}
int main(int argc,char **argv)
{
    if(argc==2 && !strcmp(argv[1],"--ignore-term")) {
        assert(signal(SIGTERM,SIG_IGN)!=SIG_ERR);
        assert(write(STDOUT_FILENO,"READY",5)==5);
        for(;;)pause();
    }
    if(argc==2 && !strcmp(argv[1],"--grandchild")) {
        pid_t child=fork();assert(child>=0);if(child)_exit(0);
        assert(signal(SIGTERM,SIG_IGN)!=SIG_ERR);
        char ready[64];int bytes=snprintf(ready,sizeof(ready),"CHILD:%ld",(long)getpid());
        assert(write(STDOUT_FILENO,ready,bytes)==bytes);for(;;)pause();
    }
    if(argc==2 && !strcmp(argv[1],"--invalid")) {
        assert(write(STDOUT_FILENO,"wrong-protocol",14)==14);return 0;
    }
    assert(argc==3 && getuid()==1300 && geteuid()==1300);
    identities();
    struct ob_observer job={0};struct ob_mounts result={0};
    char *normal[]={argv[1],NULL};assert(!ob_observer_start(&job,normal,environ,2000));
    assert(ob_observer_start(&job,normal,environ,2000)==EBUSY);
    finish(&job);assert(!ob_observer_take(&job,&result) && result.count);
    printf("uid1300 isolated native inventory worker returned %zu native records; ",result.count);
    ob_mounts_free(&result);
    char *invalid[]={argv[2],"--invalid",NULL};assert(!ob_observer_start(&job,invalid,environ,2000));
    finish(&job);assert(ob_observer_take(&job,&result)==EIO && !result.items);
    char *ignore[]={argv[2],"--ignore-term",NULL};assert(!ob_observer_start(&job,ignore,environ,1000));
    pid_t pid=job.pid;int fd=job.fd;
    uint64_t ready=ob_observer_now()+500000000ULL;
    while(job.bytes<5 && ob_observer_now()<ready){ob_observer_tick(&job);usleep(1000);}
    assert(job.bytes==5 && !memcmp(job.buffer,"READY",5));
    assert(getpgid(job.pid)==job.pid);
    finish(&job);assert(job.stopping && job.killed && !job.stuck && job.error==ETIMEDOUT);
    assert(ob_observer_take(&job,&result)==ETIMEDOUT && !result.items);
    int status;assert(waitpid(pid,&status,WNOHANG)==-1 && errno==ECHILD);
    assert(fcntl(fd,F_GETFD)==-1 && errno==EBADF);
    assert(!ob_observer_start(&job,ignore,environ,2000));
    ready=ob_observer_now()+500000000ULL;
    while(job.bytes<5 && ob_observer_now()<ready){ob_observer_tick(&job);usleep(1000);}
    assert(job.bytes==5 && !memcmp(job.buffer,"READY",5));
    assert(getpgid(job.pid)==job.pid);
    clock_failure=1;finish(&job);clock_failure=0;
    assert(job.killed && job.error==EIO && !job.result.items);
    char *grandchild[]={argv[2],"--grandchild",NULL};
    assert(!ob_observer_start(&job,grandchild,environ,1000));
    ready=ob_observer_now()+500000000ULL;
    while(job.bytes<7 && ob_observer_now()<ready){ob_observer_tick(&job);usleep(1000);}
    assert(job.bytes>=7 && job.bytes<64);char text[64]={0};memcpy(text,job.buffer,job.bytes);
    assert(!strncmp(text,"CHILD:",6));pid_t descendant=strtol(text+6,NULL,10);
    assert(descendant>0 && getpgid(descendant)==job.pid);
    finish(&job);assert(job.error==ETIMEDOUT && job.killed && !job.active);
    ready=ob_observer_now()+1000000000ULL;
    while(kill(descendant,0)==0 && ob_observer_now()<ready)usleep(1000);
    assert(kill(descendant,0)==-1 && errno==ESRCH);
    puts("native process-group deadline killed/reaped TERM-ignoring worker after grace; clock failure cancels immediately; only owned IPC closes; malformed/timeout snapshots rejected; no LKL syscall or native mount");
    return 0;
}
