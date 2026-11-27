/* SPDX-License-Identifier: MIT
 * Existing native bindat/connectat, held-directory and socket-name custody.
 * Fresh private directories/local sockets only; no FUSE/LKL/mount/device.
 */
#include "../../../lkl/guardian-protocol.h"
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <errno.h>
static int fail_chmod;
static int fault_chmod(int fd,const char *path,mode_t mode,int flags) {
    int rc=fchmodat(fd,path,mode,flags);if(!rc && fail_chmod){fail_chmod=0;errno=EIO;return -1;}return rc;
}
#define LFBG_ENDPOINT_CHMOD fault_chmod
#include "../guardian-platform.h"
#include <stdio.h>
#include <sys/wait.h>
static unsigned checks;
#define CHECK(x) do{++checks;if(!(x)){fprintf(stderr,"check %u line %u failed: %s errno=%d\n",checks,__LINE__,#x,errno);exit(1);}}while(0)
static void path(char out[PATH_MAX],const char *root,const char *name) {
    int n=snprintf(out,PATH_MAX,"%s/%s",root,name);CHECK(n>0 && n<PATH_MAX);
}
static void client(struct lfbg_endpoint *endpoint) {
    pid_t child=fork();CHECK(child>=0);
    if(!child) {
        int fd=socket(AF_LOCAL,SOCK_STREAM|SOCK_CLOEXEC,0);CHECK(fd>=0);
        struct sockaddr_un a={.sun_family=AF_LOCAL};memcpy(a.sun_path,LFBG_SOCKET_NAME,sizeof(LFBG_SOCKET_NAME));
        a.sun_len=(uint8_t)(offsetof(struct sockaddr_un,sun_path)+sizeof(LFBG_SOCKET_NAME));
        CHECK(!connectat(endpoint->directory_fd,fd,(struct sockaddr *)&a,a.sun_len));
        CHECK(write(fd,"ok",2)==2);char ack;CHECK(read(fd,&ack,1)==1 && ack=='a');CHECK(!close(fd));_exit(0);
    }
    struct pollfd ready={.fd=endpoint->listener_fd,.events=POLLIN};CHECK(poll(&ready,1,1000)==1);
    int fd=lfbg_endpoint_accept(endpoint);CHECK(fd>=0);
    CHECK((fcntl(fd,F_GETFD)&FD_CLOEXEC) && (fcntl(fd,F_GETFL)&O_NONBLOCK));
    ready=(struct pollfd){.fd=fd,.events=POLLIN};CHECK(poll(&ready,1,1000)==1);
    char bytes[2];CHECK(read(fd,bytes,2)==2 && !memcmp(bytes,"ok",2));CHECK(write(fd,"a",1)==1);CHECK(!close(fd));
    int status;CHECK(waitpid(child,&status,0)==child && WIFEXITED(status) && !WEXITSTATUS(status));
}
int main(int argc,char **argv) {
    CHECK(argc==2 && getuid()!=0 && getuid()==geteuid());
    struct stat root;CHECK(!lstat(argv[1],&root) && S_ISDIR(root.st_mode) && root.st_uid==getuid() && !(root.st_mode&0077));
    char directory[PATH_MAX],moved[PATH_MAX],alias[PATH_MAX],socket_path[PATH_MAX],old_socket[PATH_MAX];
    path(directory,argv[1],"session");path(moved,argv[1],"moved");path(alias,argv[1],"alias");
    CHECK(!mkdir(directory,0700));struct lfbg_endpoint endpoint;
    CHECK(!lfbg_endpoint_create(directory,&endpoint) && endpoint.ready);
    CHECK(!lfbg_endpoint_check(&endpoint));
    CHECK((fcntl(endpoint.directory_fd,F_GETFD)&FD_CLOEXEC) && (fcntl(endpoint.listener_fd,F_GETFD)&FD_CLOEXEC));
    CHECK(lfbg_endpoint_accept(&endpoint)<0 && (errno==EAGAIN || errno==EWOULDBLOCK));client(&endpoint);
    int held_listener=fcntl(endpoint.listener_fd,F_DUPFD_CLOEXEC,16);CHECK(held_listener>=0);
    int other=openat(endpoint.directory_fd,"fd-replacement",O_RDWR|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);CHECK(other>=0);
    CHECK(dup2(other,endpoint.listener_fd)==endpoint.listener_fd);
    CHECK(lfbg_endpoint_check(&endpoint)<0 && errno==ESTALE);
    CHECK(lfbg_endpoint_close(&endpoint)<0 && errno==ESTALE);
    CHECK(fcntl(other,F_GETFD)>=0 && fcntl(endpoint.listener_fd,F_GETFD)>=0);
    /* Restore the still-held same socket, rather than adopting a new one. */
    CHECK(dup2(held_listener,endpoint.listener_fd)==endpoint.listener_fd);
    CHECK(!fcntl(endpoint.listener_fd,F_SETFD,FD_CLOEXEC));
    CHECK(!close(held_listener) && !close(other) && !lfbg_endpoint_check(&endpoint));
    struct lfbg_endpoint duplicate;CHECK(lfbg_endpoint_create(directory,&duplicate)<0 && errno==EADDRINUSE && duplicate.directory_fd==-1 && duplicate.listener_fd==-1);

    CHECK(!chmod(directory,0750));CHECK(lfbg_endpoint_check(&endpoint)<0 && errno==ESTALE);
    CHECK(lfbg_endpoint_close(&endpoint)<0 && errno==ESTALE);CHECK(!chmod(directory,0700));
    CHECK(!rename(directory,moved) && !mkdir(directory,0700));
    CHECK(lfbg_endpoint_check(&endpoint)<0 && errno==ESTALE);
    CHECK(lfbg_endpoint_accept(&endpoint)<0 && errno==ESTALE);
    CHECK(lfbg_endpoint_close(&endpoint)<0 && errno==ESTALE);
    struct stat st;CHECK(!fstatat(endpoint.directory_fd,LFBG_SOCKET_NAME,&st,AT_SYMLINK_NOFOLLOW) && st.st_ino==endpoint.socket.st_ino);
    /* Explicit test restoration of the original directory, never adoption. */
    CHECK(!rmdir(directory) && !rename(moved,directory) && !lfbg_endpoint_check(&endpoint));

    path(socket_path,directory,LFBG_SOCKET_NAME);path(old_socket,directory,"original.sock");CHECK(!rename(socket_path,old_socket));
    int replacement=socket(AF_LOCAL,SOCK_STREAM|SOCK_CLOEXEC,0);CHECK(replacement>=0);
    struct sockaddr_un a={.sun_family=AF_LOCAL};memcpy(a.sun_path,LFBG_SOCKET_NAME,sizeof(LFBG_SOCKET_NAME));
    a.sun_len=(uint8_t)(offsetof(struct sockaddr_un,sun_path)+sizeof(LFBG_SOCKET_NAME));
    CHECK(!bindat(endpoint.directory_fd,replacement,(struct sockaddr *)&a,a.sun_len));CHECK(!chmod(socket_path,0600));
    CHECK(!lstat(socket_path,&st));ino_t replacement_inode=st.st_ino;
    CHECK(lfbg_endpoint_check(&endpoint)<0 && errno==ESTALE);
    CHECK(lfbg_endpoint_close(&endpoint)<0 && errno==ESTALE);
    CHECK(!lstat(socket_path,&st) && st.st_ino==replacement_inode);
    CHECK(!unlink(socket_path) && !close(replacement) && !rename(old_socket,socket_path));
    CHECK(!lfbg_endpoint_check(&endpoint));client(&endpoint);
    CHECK(!lfbg_endpoint_close(&endpoint) && endpoint.directory_fd==-1 && endpoint.listener_fd==-1 && !endpoint.ready);
    CHECK(lstat(socket_path,&st)<0 && errno==ENOENT);

    CHECK(!symlink(directory,alias));CHECK(lfbg_endpoint_create(alias,&duplicate)<0 && errno==EPERM);CHECK(!unlink(alias));
    CHECK(!chmod(directory,0755));CHECK(lfbg_endpoint_create(directory,&duplicate)<0 && errno==EPERM);CHECK(!chmod(directory,0700));
    int file=open(socket_path,O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW,0600);CHECK(file>=0 && write(file,"keep",4)==4 && !close(file));
    CHECK(lfbg_endpoint_create(directory,&duplicate)<0 && errno==EADDRINUSE);
    file=open(socket_path,O_RDONLY|O_NOFOLLOW);char bytes[4];CHECK(file>=0 && read(file,bytes,4)==4 && !memcmp(bytes,"keep",4) && !close(file));CHECK(!unlink(socket_path));

    fail_chmod=1;CHECK(lfbg_endpoint_create(directory,&endpoint)==1 && errno==EIO && !fail_chmod && !endpoint.ready);
    CHECK(endpoint.directory_fd>=0 && endpoint.listener_fd>=0 && !lfbg_endpoint_check(&endpoint));
    /* No service/worker was launched: reconcile this exact quiescent fixture. */
    CHECK(!lfbg_endpoint_close(&endpoint));
    printf("{\"result\":\"pass\",\"checks\":%u,\"scope\":\"native held-directory endpoint custody, no-clobber bindat, accept, replacement/move refusal and retained publication error; local sockets only\"}\n",checks);return 0;
}
