/* SPDX-License-Identifier: GPL-2.0-only */
#include "openbsd-caller.h"
#include <endian.h>
#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>
#include <sys/wait.h>
#include <lkl.h>
static int fail_restore;
static long controlled_uid(lkl_uid_t r,lkl_uid_t e,lkl_uid_t s)
{
    if(fail_restore && !r && !e && !s){fail_restore=0;return -LKL_EIO;}
    return lkl_sys_setresuid(r,e,s);
}
#define lkl_sys_setresuid controlled_uid
#include "openbsd-credentials.h"
#undef lkl_sys_setresuid
#include "openbsd-handles.h"
#include "openbsd-errno.h"
#include <lkl_host.h>
static struct ob_credentials state;
static void *wrong_thread(void *unused)
{(void)unused;assert(ob_restore_credentials(&state)==EPERM);return NULL;}
int main(int argc,char **argv)
{
    assert(argc==2 && getuid()==1300 && geteuid()==1300);setvbuf(stdout,NULL,_IONBF,0);
    int source=open(argv[1],O_CREAT|O_EXCL|O_RDWR|O_NOFOLLOW|O_CLOEXEC,0600);assert(source>=0);
    int channel[2];assert(!pipe(channel));pid_t child=fork();assert(child>=0);
    if(!child){close(channel[0]);pid_t tid=getthrid();assert(write(channel[1],&tid,sizeof(tid))==sizeof(tid));close(channel[1]);alarm(30);for(;;)pause();}
    close(channel[1]);pid_t tid;assert(read(channel[0],&tid,sizeof(tid))==sizeof(tid));close(channel[0]);
    struct ob_caller caller;assert(!ob_stable_caller(tid,getuid(),getgid(),&caller) && caller.pid==child);
    lkl_host_ops.print=NULL;assert(!lkl_init(&lkl_host_ops));assert(!lkl_start_kernel("mem=32M"));assert(!ob_credentials_init(&state));
    assert(!lkl_sys_mkdir("/owned",0700));assert(!lkl_sys_chown("/owned",1300,1300));
    assert(!lkl_sys_mkdir("/groups",0700));assert(!lkl_sys_chown("/groups",32766,0));assert(!lkl_sys_chmod("/groups",0070));
    pthread_t t;assert(!pthread_create(&t,NULL,wrong_thread,NULL));assert(!pthread_join(t,NULL));
    assert(!ob_enter_credentials(&state,&caller,0077));assert(ob_enter_credentials(&state,&caller,0)==EDEADLK);
    long fd=lkl_sys_open("/owned/held",LKL_O_CREAT|LKL_O_EXCL|LKL_O_RDWR,0666);assert(fd>=0);
    struct lkl_stat st;assert(!lkl_sys_fstat(fd,&st));assert(st.st_uid==1300 && st.st_gid==1300 && (st.st_mode&07777)==0600);
    assert(lkl_sys_fchown(fd,0,(lkl_gid_t)-1)==-LKL_EPERM);assert(!lkl_sys_fchmod(fd,06750));assert(lkl_sys_write(fd,"A",1)==1);assert(!lkl_sys_fstat(fd,&st) && !(st.st_mode&06000));
    long groupfd=lkl_sys_open("/groups/group-only",LKL_O_CREAT|LKL_O_EXCL|LKL_O_RDWR,0600);assert(groupfd>=0);assert(!lkl_sys_close(groupfd));
    assert(!ob_restore_credentials(&state));
    struct ob_caller foreign=caller;foreign.uid=32767;foreign.gid=32767;foreign.ngroups=1;foreign.groups[0]=32767;
    assert(!ob_enter_credentials(&state,&foreign,0));assert(lkl_sys_open("/owned/held",LKL_O_RDONLY,0)==-LKL_EACCES);assert(lkl_sys_open("/groups/group-only",LKL_O_RDONLY,0)==-LKL_EACCES);assert(!ob_restore_credentials(&state));
    assert(!ob_enter_credentials(&state,&caller,0));long modefd=lkl_sys_open("/owned/native-mode",LKL_O_CREAT|LKL_O_EXCL|LKL_O_RDWR,0644);assert(modefd>=0);assert(!lkl_sys_fstat(modefd,&st) && (st.st_mode&07777)==0644);assert(!lkl_sys_close(modefd));
    struct ob_handles handles={.next=1};uint64_t token;assert(!ob_register_handle(&handles,fd,LKL_O_RDWR,OB_FILE,&caller,&token));
    uint64_t unused;assert(ob_register_handle(&handles,fd,LKL_O_RDWR,OB_FILE,&caller,&unused)==EEXIST);
    assert(!kill(child,SIGTERM));int status;assert(waitpid(child,&status,0)==child);
    struct ob_caller dead;assert(ob_stable_caller(tid,getuid(),getgid(),&dead)==ESRCH);
    assert(!ob_find_handle(&handles,token,OB_DIRECTORY));assert(!ob_find_handle(&handles,0,OB_FILE));assert(!ob_find_handle(&handles,fd+1000,OB_FILE));
    assert(!lkl_sys_mkdir("/owned/notempty",0700));long childfd=lkl_sys_open("/owned/notempty/child",LKL_O_CREAT|LKL_O_WRONLY,0600);assert(childfd>=0);assert(!lkl_sys_close(childfd));
    assert(ob_lkl_result(lkl_sys_rmdir("/owned/notempty"),0)==-ENOTEMPTY);
    fail_restore=1;assert(ob_restore_credentials(&state)==EIO && state.poison==EIO && state.active);
    assert(ob_enter_credentials(&state,&caller,0)==EIO && fcntl(source,F_GETFD)>=0);
    assert(!ob_handle_barrier(&handles,token,OB_FILE));assert(!ob_release_handle(&handles,token,OB_FILE));assert(ob_release_handle(&handles,token,OB_FILE)==EBADF);
    assert(!ob_restore_credentials(&state));assert(state.poison==EIO && ob_enter_credentials(&state,&caller,0)==EIO);
    assert(ob_credentials_init(&state)==EALREADY && state.poison==EIO);
    assert(!lkl_sys_halt());lkl_cleanup();assert(!close(source));
    puts("native stable caller -> RAM real/effective/fsUID/GID/fullgroups/umask verified; group-only/foreign/root-denial/setID/native-errno gates; single-thread ownership; poisoned new operations denied, registered FD-only barrier/release survived and remained sticky; no mount");
}
