/* SPDX-License-Identifier: GPL-2.0-only */
#include "openbsd-caller.h"
#include <assert.h>
#include <pthread.h>
#include <stdio.h>
#include <unistd.h>
#include <sys/wait.h>

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ready = PTHREAD_COND_INITIALIZER;
static pid_t thread_id;
static int done;
static void *thread(void *unused)
{
    (void)unused;
    assert(!pthread_mutex_lock(&lock)); thread_id=getthrid();
    assert(!pthread_cond_signal(&ready));
    while (!done) assert(!pthread_cond_wait(&ready,&lock));
    assert(!pthread_mutex_unlock(&lock)); return NULL;
}
static struct ob_caller fake;
static unsigned fake_calls, fake_change;
static int changed_snapshot(pid_t tid,uid_t uid,gid_t gid,struct ob_caller *out)
{
    (void)tid;(void)uid;(void)gid; *out=fake;
    if (++fake_calls==2) {
        if(fake_change==1)out->birth_usec^=1;
        if(fake_change==2)out->groups[out->ngroups-1]^=1;
        if(fake_change==3)out->thrid++;
    }
    return 0;
}
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);
    setvbuf(stdout,NULL,_IONBF,0);
    struct ob_caller self, other;
    assert(!ob_stable_caller(getthrid(),getuid(),getgid(),&self));
    assert(self.pid==getpid() && self.thrid==getthrid() && self.thrid!=self.pid);
    gid_t groups[KI_NGROUPS]; int n=getgroups(KI_NGROUPS,groups);assert(n>=0);
    for(int i=0;i<n;i++) {int present=0;for(unsigned j=0;j<self.ngroups;j++)present|=groups[i]==self.groups[j];assert(present);}
    assert(ob_stable_caller(getpid(),getuid(),getgid(),&other)==ESRCH);
    assert(ob_stable_caller(0,getuid(),getgid(),&other)==ESRCH);
    assert(ob_stable_caller(getthrid(),32767,getgid(),&other)==EACCES);
    assert(ob_stable_caller(getthrid(),getuid(),32767,&other)==EACCES);
    pthread_t worker; assert(!pthread_mutex_lock(&lock));assert(!pthread_create(&worker,NULL,thread,NULL));
    while(!thread_id)assert(!pthread_cond_wait(&ready,&lock));
    assert(!ob_stable_caller(thread_id,getuid(),getgid(),&other));
    assert(other.pid==self.pid && other.thrid!=self.thrid && other.birth_sec==self.birth_sec && other.birth_usec==self.birth_usec);
    done=1;assert(!pthread_cond_signal(&ready));assert(!pthread_mutex_unlock(&lock));assert(!pthread_join(worker,NULL));
    assert(ob_stable_caller(thread_id,getuid(),getgid(),&other)==ESRCH);
    int fds[2];assert(!pipe(fds));pid_t child=fork();assert(child>=0);
    if(!child){close(fds[0]);pid_t tid=getthrid();assert(write(fds[1],&tid,sizeof(tid))==sizeof(tid));close(fds[1]);sleep(10);_exit(0);}
    close(fds[1]);pid_t tid;assert(read(fds[0],&tid,sizeof(tid))==sizeof(tid));close(fds[0]);
    assert(!ob_stable_caller(tid,getuid(),getgid(),&other) && other.pid==child);
    assert(!kill(child,SIGTERM));int status;assert(waitpid(child,&status,0)==child);
    assert(ob_stable_caller(tid,getuid(),getgid(),&other)==ESRCH);
    fake=self;
    for(fake_change=1;fake_change<=3;fake_change++) {fake_calls=0;assert(ob_stable_caller_with(changed_snapshot,getthrid(),getuid(),getgid(),&other)==EAGAIN);}
    struct kinfo_proc rows[2]={0};rows[0].p_tid=getthrid();rows[0].p_pid=getpid();rows[0].p_uid=getuid();rows[0].p_gid=getgid();rows[0].p_stat=SRUN;rows[0].p_uvalid=1;rows[0].p_ustart_sec=1;rows[0].p_ngroups=1;rows[0].p_groups[0]=getgid();
    assert(!ob_caller_records(rows,sizeof(rows[0]),getthrid(),getuid(),getgid(),&other));
    rows[1]=rows[0];assert(ob_caller_records(rows,sizeof(rows),getthrid(),getuid(),getgid(),&other)==EIO);
    assert(ob_caller_records(rows,sizeof(rows)-1,getthrid(),getuid(),getgid(),&other)==EIO);
    rows[0].p_ngroups=KI_NGROUPS+1;assert(ob_caller_records(rows,sizeof(rows[0]),getthrid(),getuid(),getgid(),&other)==EOVERFLOW);
    rows[0].p_ngroups=1;rows[0].p_stat=SDEAD;assert(ob_caller_records(rows,sizeof(rows[0]),getthrid(),getuid(),getgid(),&other)==ESRCH);
    printf("native encodedTHRID=%d processPID=%d threads/child/reap/fullgroups=%u stable-birth/group/THRID-change and malformed-record rejection passed\n",self.thrid,self.pid,self.ngroups);
}
