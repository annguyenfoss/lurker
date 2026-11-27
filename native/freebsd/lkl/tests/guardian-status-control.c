/* SPDX-License-Identifier: MIT
 * Native user-only regular status-file persistence and late-error controls.
 * Visible clean bytes are deliberately shown to be insufficient authority.
 */
#include "../../../lkl/guardian-protocol.h"
#include <sys/types.h>
#include <unistd.h>
#include <errno.h>
static int sync_error,close_error,partial_error;
static int fault_sync(int fd) {
    int rc=fsync(fd);if(!rc && sync_error){sync_error=0;errno=EIO;return -1;}return rc;
}
static int fault_close(int fd) {
    int rc=close(fd);if(!rc && close_error){close_error=0;errno=EIO;return -1;}return rc;
}
static ssize_t fault_write(int fd,const void *bytes,size_t size,off_t offset) {
    if(partial_error==1){partial_error=2;return pwrite(fd,bytes,size/2,offset);}
    if(partial_error==2){partial_error=0;errno=EIO;return -1;}
    return pwrite(fd,bytes,size,offset);
}
#define LFBG_STATUS_FSYNC fault_sync
#define LFBG_STATUS_CLOSE fault_close
#define LFBG_STATUS_PWRITE fault_write
#include "../guardian-platform.h"
#include <sys/file.h>
#include <sys/wait.h>
#include <stdio.h>
static unsigned checks;
#define CHECK(x) do { ++checks;if(!(x)){fprintf(stderr,"check %u line %u failed: %s errno=%d\n",checks,__LINE__,#x,errno);exit(1);} } while(0)
static int descriptor_count(void){int n=0;for(int fd=0;fd<256;++fd)if(fcntl(fd,F_GETFD)>=0)++n;return n;}
static struct lfg_message golden(const char *path) {
    FILE *f=fopen(path,"r");CHECK(f!=NULL);uint8_t bytes[LFG_FRAME_SIZE];unsigned x;
    for(size_t i=0;i<sizeof(bytes);++i){CHECK(fscanf(f,"%2x",&x)==1);bytes[i]=(uint8_t)x;}
    CHECK(fscanf(f,"%2x",&x)==EOF && !fclose(f));
    struct lfg_message m;CHECK(lfg_decode(&m,bytes,sizeof(bytes))==LFG_OK && lfg_clean_candidate(&m));return m;
}
static void still_locked(const char *path) {
    pid_t child=fork();CHECK(child>=0);
    if(!child){int fd=open(path,O_RDWR|O_NOFOLLOW|O_CLOEXEC);CHECK(fd>=0);CHECK(flock(fd,LOCK_EX|LOCK_NB)<0 && errno==EWOULDBLOCK);CHECK(!close(fd));_exit(0);}
    int status;CHECK(waitpid(child,&status,0)==child && WIFEXITED(status) && !WEXITSTATUS(status));
}
static void visible_clean(int fd,const struct lfg_message *expected) {
    uint8_t bytes[LFG_FRAME_SIZE];struct lfg_message m;
    CHECK(pread(fd,bytes,sizeof(bytes),0)==sizeof(bytes));
    CHECK(lfg_decode(&m,bytes,sizeof(bytes))==LFG_OK && lfg_clean_candidate(&m));
    CHECK(!memcmp(&m,expected,sizeof(m)));
}
int main(int argc,char **argv) {
    CHECK(argc==3 && getuid()!=0 && getuid()==geteuid());
    struct stat dir;CHECK(!lstat(argv[1],&dir) && S_ISDIR(dir.st_mode) && dir.st_uid==getuid() && !(dir.st_mode&0077));
    struct lfg_message message=golden(argv[2]);char path[PATH_MAX],other[PATH_MAX],linkpath[PATH_MAX];
    snprintf(path,sizeof(path),"%s/status",argv[1]);snprintf(other,sizeof(other),"%s/other",argv[1]);snprintf(linkpath,sizeof(linkpath),"%s/link",argv[1]);
    int fd=open(path,O_RDWR|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);CHECK(fd>=0);
    CHECK(!flock(fd,LOCK_EX|LOCK_NB));struct lfbg_status_identity identity;
    CHECK(!lfbg_status_capture(fd,&identity));
    CHECK(!lfbg_status_persist(fd,&identity,&message));visible_clean(fd,&message);still_locked(path);
    int descriptors=descriptor_count();
    sync_error=1;CHECK(lfbg_status_persist(fd,&identity,&message)<0 && errno==EIO && !sync_error);
    CHECK(descriptor_count()==descriptors);visible_clean(fd,&message);still_locked(path);
    close_error=1;CHECK(lfbg_status_persist(fd,&identity,&message)<0 && errno==EIO && !close_error);
    CHECK(descriptor_count()==descriptors);visible_clean(fd,&message);still_locked(path);
    partial_error=1;CHECK(lfbg_status_persist(fd,&identity,&message)<0 && errno==EIO && !partial_error);
    CHECK(descriptor_count()==descriptors && fcntl(fd,F_GETFD)>=0);still_locked(path);
    CHECK(!lfbg_status_persist(fd,&identity,&message));visible_clean(fd,&message);
    CHECK(!link(path,linkpath));CHECK(lfbg_status_persist(fd,&identity,&message)<0 && errno==EPERM);CHECK(!unlink(linkpath));
    CHECK(!fchmod(fd,0644));CHECK(lfbg_status_persist(fd,&identity,&message)<0 && errno==EPERM);CHECK(!fchmod(fd,0600));
    int readonly=open(path,O_RDONLY|O_CLOEXEC|O_NOFOLLOW);CHECK(readonly>=0);
    CHECK(lfbg_status_capture(readonly,&identity)<0 && errno==EPERM);CHECK(!close(readonly));
    CHECK(!lfbg_status_capture(fd,&identity));
    int replacement=open(other,O_RDWR|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);CHECK(replacement>=0);
    CHECK(lfbg_status_persist(replacement,&identity,&message)<0 && errno==ESTALE);CHECK(!close(replacement));
    CHECK(!ftruncate(fd,LFG_FRAME_SIZE+1));CHECK(lfbg_status_persist(fd,&identity,&message)<0 && errno==EPERM);CHECK(!ftruncate(fd,LFG_FRAME_SIZE));
    struct lfg_message malformed=message;malformed.kind=0;CHECK(lfbg_status_persist(fd,&identity,&malformed)<0 && errno==EPROTO);
    CHECK(descriptor_count()==descriptors);CHECK(!lfbg_status_check(fd,&identity));still_locked(path);
    CHECK(!close(fd));
    printf("{\"result\":\"pass\",\"checks\":%u,\"scope\":\"guardian-only status writer, visible-clean fsync/close failures, partial writes, custody lock and identity; regular files/dummy children only\"}\n",checks);return 0;
}
