/* SPDX-License-Identifier: GPL-2.0-only */
#define _NETBSD_SOURCE
#include "netbsd-caller.h"
#include <sys/endian.h>
#include <sys/wait.h>
#include <assert.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <lkl.h>
static int restore_failure;
static long controlled_setresuid(unsigned int real,unsigned int effective,unsigned int saved)
{
    if (restore_failure && !real && !effective && !saved) {
        restore_failure=0;
        return -LKL_EIO;
    }
    return lkl_sys_setresuid(real,effective,saved);
}
#define lkl_sys_setresuid controlled_setresuid
#include "netbsd-credentials.h"
#undef lkl_sys_setresuid
#include <lkl_host.h>

int main(int argc,char **argv)
{
    assert(argc==2 && getuid()==1300 && geteuid()==1300);
    setvbuf(stdout,NULL,_IONBF,0);
    int source=open(argv[1],O_RDWR|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);assert(source>=0);
    pid_t child=fork();assert(child>=0);
    if(!child){alarm(20);for(;;)pause();}
    struct netbsd_credential credential={.umask=0077};
    int error=EIO;
    for(int i=0;i<100;i++) {
        error=netbsd_stable_caller(child,getuid(),getgid(),&credential.caller);
        if(!error)break;
        usleep(10000);
    }
    assert(!error);
    struct netbsd_caller invalid;
    assert(netbsd_stable_caller(0,getuid(),getgid(),&invalid)==ESRCH);
    assert(netbsd_stable_caller(getpid(),32767,getgid(),&invalid)==EACCES);
    lkl_host_ops.print=NULL;assert(!lkl_init(&lkl_host_ops));assert(!lkl_start_kernel("mem=64M"));
    assert(!netbsd_restore_credentials());
    assert(!lkl_sys_mkdir("/owned",0700));assert(!lkl_sys_chown("/owned",1300,1300));
    assert(!netbsd_enter_credentials(&credential,false));
    long fd=lkl_sys_open("/owned/held",LKL_O_CREAT|LKL_O_EXCL|LKL_O_RDWR,0666);assert(fd>=0);
    struct lkl_stat st;assert(!lkl_sys_fstat(fd,&st));
    assert(st.st_uid==1300 && st.st_gid==1300 && (st.st_mode&07777)==0600);
    assert(lkl_sys_fchown(fd,0,(unsigned)-1)==-LKL_EPERM);
    assert(!lkl_sys_fchmod(fd,06750));assert(lkl_sys_write(fd,"A",1)==1);
    assert(!lkl_sys_fstat(fd,&st) && !(st.st_mode&06000));
    assert(!netbsd_restore_credentials());
    credential.umask=0;
    assert(!netbsd_enter_credentials(&credential,false));
    long modefd=lkl_sys_open("/owned/native-masked",LKL_O_CREAT|LKL_O_EXCL|LKL_O_RDWR,0644);assert(modefd>=0);
    assert(!lkl_sys_fstat(modefd,&st) && (st.st_mode&07777)==0644);assert(!lkl_sys_close(modefd));
    modefd=lkl_sys_open("/owned/explicit-no-mask",LKL_O_CREAT|LKL_O_EXCL|LKL_O_RDWR,0666);assert(modefd>=0);
    assert(!lkl_sys_fstat(modefd,&st) && (st.st_mode&07777)==0666);assert(!lkl_sys_close(modefd));
    assert(!netbsd_restore_credentials());
    credential.umask=01000;assert(netbsd_enter_credentials(&credential,false)==EINVAL);credential.umask=0;
    assert(!netbsd_credentials_active && lkl_sys_geteuid()==0 && !lkl_sys_getgroups(0,NULL));
    assert(!kill(child,SIGTERM));int status;assert(waitpid(child,&status,0)==child);
    assert(!netbsd_enter_credentials(&credential,false));
    assert(lkl_sys_pwrite64(fd,"B",1,1)==1);assert(lkl_sys_fchown(fd,0,(unsigned)-1)==-LKL_EPERM);
    restore_failure=1;assert(netbsd_restore_credentials()==EIO);
    assert(netbsd_credential_poison==EIO && netbsd_credentials_active && !restore_failure);
    assert(netbsd_enter_credentials(&credential,false)==EIO);
    assert(fcntl(source,F_GETFD)>=0);
    puts("restoration failure poisoned new callbacks without exit, channel/source close or cleanup; native source FD remains held");
    assert(!netbsd_enter_held_cleanup(&credential));
    assert(lkl_sys_geteuid()==1300 && lkl_sys_fchown(fd,0,(unsigned)-1)==-LKL_EPERM);
    assert(!lkl_sys_close(fd));assert(!netbsd_restore_credentials());
    assert(netbsd_credential_poison==EIO && netbsd_enter_credentials(&credential,false)==EIO);
    assert(lkl_sys_getuid()==0 && lkl_sys_geteuid()==0 && !lkl_sys_getgid() && !lkl_sys_getegid() && !lkl_sys_getgroups(0,NULL) && !lkl_sys_umask(0));
    assert(!lkl_sys_halt());lkl_cleanup();assert(!close(source));
    puts("uid1300 full native groups, captured dead-opener authority, ownership/root denial/set-ID clearing, umask0077/zero/native-mode preservation, cleanup-only recovery and sticky poison passed; RAM only, no native mount");
    return 0;
}
