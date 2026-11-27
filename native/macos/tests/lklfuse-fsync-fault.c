// SPDX-License-Identifier: GPL-2.0-only
/* Login-user DYLD interposer for one-shot backing-fsync fault controls only.
 * Match the exact owned fixture inode; arm explicitly, fail once, then forward
 * all later calls. Never install or inject this library into privileged code. */
#define _DARWIN_C_SOURCE
#include <sys/stat.h>
#include <sys/syscall.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
static struct stat target;
static int ready,failed;
static const char *arm,*marker;
__attribute__((constructor)) static void initialize(void){
 const char *path=getenv("LURKER_TEST_FSYNC_TARGET");arm=getenv("LURKER_TEST_FSYNC_ARM");marker=getenv("LURKER_TEST_FSYNC_MARKER");
 ready=path&&arm&&marker&&!lstat(path,&target)&&S_ISREG(target.st_mode)&&target.st_uid==getuid();
}
static int fault_fsync(int fd){
 struct stat info;int matches=ready&&!fstat(fd,&info)&&info.st_dev==target.st_dev&&info.st_ino==target.st_ino;
 if(matches&&!access(arm,F_OK)&&__sync_bool_compare_and_swap(&failed,0,1)){
  int out=open(marker,O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);if(out>=0){(void)write(out,"one-shot EIO\n",13);(void)close(out);}
  fprintf(stderr,"INJECT_HOST_FSYNC once=1 fd=%d errno=%d\n",fd,EIO);errno=EIO;return -1;
 }
 /* Direct native syscall avoids re-interposing dlsym(RTLD_NEXT) on Darwin. */
 int result=(int)syscall(SYS_fsync,fd),error=errno;
 if(matches&&__atomic_load_n(&failed,__ATOMIC_ACQUIRE))fprintf(stderr,"AFTER_INJECT_HOST_FSYNC fd=%d result=%d errno=%d armed=%d\n",fd,result,result?error:0,!access(arm,F_OK));
 errno=error;return result;
}
/* Public Mach-O dyld interpose-pair layout; the optional SDK convenience
 * header is not shipped by every Xcode release. */
__attribute__((used,section("__DATA,__interpose")))
static const struct {const void *replacement;const void *original;} interpose_fsync = {
 (const void *)fault_fsync,(const void *)fsync
};
