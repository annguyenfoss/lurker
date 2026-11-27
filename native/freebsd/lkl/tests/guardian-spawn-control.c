/* SPDX-License-Identifier: MIT
 * Native user-only production-FD controls. Regular files and dummy children;
 * no LKL, FUSE, mount, provider ioctl or encryption operation.
 */
#include "../../../lkl/guardian-protocol.h"
#include <unistd.h>
#include <errno.h>
#include <sys/stat.h>
static int fail_one_close;
static int alter_exec_after;
static int controlled_close(int fd) {
    int rc = close(fd);
    if (!rc && fail_one_close) {
        fail_one_close = 0; errno = EIO; return -1;
    }
    return rc;
}
#define LFBG_CLOSE_INTERNAL controlled_close
static int controlled_exec_stat(const char *path,struct stat *st) {
    int rc=stat(path,st);
    if (!rc && alter_exec_after && !--alter_exec_after) ++st->st_ino;
    return rc;
}
#define LFBG_EXEC_STAT controlled_exec_stat
#include "../guardian-platform.h"
#include <stdio.h>
#include <pthread.h>
#include <stdatomic.h>
#include <sys/wait.h>
extern char **environ;
static unsigned checks;
#define CHECK(x) do { ++checks; if (!(x)) { \
 fprintf(stderr,"check %u line %u failed: %s errno=%d\n",checks,__LINE__,#x,errno);exit(1); } } while(0)
static atomic_int thread_wait;
static void *wait_thread(void *unused) {
    (void)unused;
    while (atomic_load(&thread_wait)) usleep(1000);
    return NULL;
}
static int worker(const char *inode) {
    struct stat st, log, err;
    sigset_t mask;
    CHECK(!lfbg_worker_fds_prepare());
    CHECK((fcntl(3,F_GETFD)&FD_CLOEXEC) && (fcntl(4,F_GETFD)&FD_CLOEXEC));
    CHECK(fcntl(5,F_GETFD)<0 && errno==EBADF);
    CHECK(fcntl(100,F_GETFD)<0 && errno==EBADF);
    CHECK(!fstat(4,&st) && S_ISREG(st.st_mode) && st.st_size==4096);
    CHECK((uint64_t)st.st_ino==strtoull(inode,NULL,10));
    CHECK(!fstat(1,&log) && !fstat(2,&err) && S_ISREG(log.st_mode));
    CHECK(log.st_ino==err.st_ino && log.st_dev==err.st_dev);
    CHECK(!sigprocmask(SIG_SETMASK,NULL,&mask) && !sigismember(&mask,SIGTERM));
    CHECK(getpgrp()==getpid());
    char byte;
    CHECK(read(0,&byte,1)==0);
    printf("dummy worker verified channel3/source4 only\n");
    CHECK(!fflush(stdout));
    CHECK(write(3,"ok",2)==2 && read(3,&byte,1)==1 && byte=='a');
    return 0;
}
static void finish_child(pid_t child, int channel) {
    char bytes[2];
    CHECK(read(channel,bytes,2)==2 && !memcmp(bytes,"ok",2));
    CHECK(write(channel,"a",1)==1);
    int status;
    CHECK(waitpid(child,&status,0)==child && WIFEXITED(status) && !WEXITSTATUS(status));
}
int main(int argc,char **argv) {
    if (argc==3 && !strcmp(argv[1],"--worker")) return worker(argv[2]);
    CHECK(argc==2 && getuid()!=0 && getuid()==geteuid());
    struct stat dir;
    CHECK(!lstat(argv[1],&dir) && S_ISDIR(dir.st_mode) && dir.st_uid==getuid() && !(dir.st_mode&0077));
    char self[PATH_MAX], path[PATH_MAX];
    CHECK(realpath(argv[0],self));
    snprintf(path,sizeof(path),"%s/source",argv[1]);
    int source=open(path,O_RDWR|O_CREAT|O_EXCL|O_NOFOLLOW,0600);
    CHECK(source==3 && !ftruncate(source,4096));
    char sourcepath[PATH_MAX];strcpy(sourcepath,path);
    snprintf(path,sizeof(path),"%s/status",argv[1]);
    int status=open(path,O_RDWR|O_CREAT|O_EXCL|O_NOFOLLOW,0600);CHECK(status==4);
    snprintf(path,sizeof(path),"%s/log",argv[1]);
    int log=open(path,O_RDWR|O_CREAT|O_EXCL|O_NOFOLLOW,0600);CHECK(log==5);
    int pair[2];CHECK(!socketpair(AF_UNIX,SOCK_STREAM|SOCK_CLOEXEC,0,pair));
    CHECK(dup2(source,100)==100 && !(fcntl(100,F_GETFD)&FD_CLOEXEC));
    struct stat st;CHECK(!fstat(source,&st));char inode[32];
    snprintf(inode,sizeof(inode),"%llu",(unsigned long long)st.st_ino);
    char *args[]={self,"--worker",inode,NULL};
    struct lfbg_spawn_inputs in={self,args,environ,pair[1],source,status,log};
    pid_t child;
    struct lfbg_spawn_inputs bad=in;bad.status_fd=source;
    CHECK(lfbg_spawn_worker(&bad,&child)<0 && child==-1 && errno==EINVAL);
    char missing[PATH_MAX];
    snprintf(missing,sizeof(missing),"%s/missing-executable",argv[1]);
    char *missing_args[]={missing,NULL};
    bad=in;bad.executable=missing;bad.argv=missing_args;
    CHECK(lfbg_spawn_worker(&bad,&child)<0 && child==-1 && errno==ENOENT);
    bad=in;bad.log_fd=status;
    CHECK(lfbg_spawn_worker(&bad,&child)<0 && child==-1);
    CHECK(!fchmod(status,0644));
    CHECK(lfbg_spawn_worker(&in,&child)<0 && child==-1 && errno==EPERM);
    CHECK(!fchmod(status,0600));
    int readonly=open(sourcepath,O_RDONLY|O_CLOEXEC);CHECK(readonly>=0);
    bad=in;bad.source_fd=readonly;
    CHECK(lfbg_spawn_worker(&bad,&child)<0 && child==-1 && errno==EINVAL);
    CHECK(!close(readonly));
    atomic_store(&thread_wait,1);pthread_t thread;
    CHECK(!pthread_create(&thread,NULL,wait_thread,NULL));
    CHECK(lfbg_spawn_worker(&in,&child)<0 && child==-1 && errno==EBUSY);
    atomic_store(&thread_wait,0);CHECK(!pthread_join(thread,NULL));
    sigset_t blocked,previous;sigemptyset(&blocked);sigaddset(&blocked,SIGTERM);
    CHECK(!sigprocmask(SIG_BLOCK,&blocked,&previous));
    CHECK(!lfbg_spawn_worker(&in,&child) && child>0);
    CHECK((fcntl(status,F_GETFD)&FD_CLOEXEC) && fcntl(source,F_GETFD)>=0);
    finish_child(child,pair[0]);
    CHECK(!sigprocmask(SIG_SETMASK,&previous,NULL));
    fail_one_close=1;
    CHECK(lfbg_spawn_worker(&in,&child)==LFBG_SPAWN_RETAINED && child>0 && errno==EIO);
    finish_child(child,pair[0]);
    CHECK(!fail_one_close && fcntl(status,F_GETFD)>=0 && fcntl(source,F_GETFD)>=0);
    alter_exec_after=2;
    CHECK(lfbg_spawn_worker(&in,&child)==LFBG_SPAWN_RETAINED && child>0 && errno==ESTALE);
    finish_child(child,pair[0]);
    CHECK(!alter_exec_after);
    CHECK(pwrite(status,"guardian-only",13,0)==13);
    CHECK(!close(source) && !close(status) && !close(log));
    CHECK(!close(pair[0]) && !close(pair[1]) && !close(100));
    printf("{\"result\":\"pass\",\"checks\":%u,\"scope\":\"native production FD3/4 spawn, status exclusion, pre-thread refusal and post-spawn cleanup error; regular files/dummy children only\"}\n",checks);
    return 0;
}
