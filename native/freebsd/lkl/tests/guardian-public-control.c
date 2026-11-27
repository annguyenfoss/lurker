/* SPDX-License-Identifier: MIT
 * User-only actual AF_LOCAL listener/accept first-write credential controls.
 * No LKL, FUSE, mount, device, privilege or encryption operation.
 */
#include "../../../lkl/guardian-protocol.h"
#include "../guardian-platform.h"
#include <stdio.h>
#include <sys/wait.h>
static unsigned checks;
#define CHECK(x) do {++checks;if(!(x)){fprintf(stderr,"check %u line %u failed: %s errno=%d\n",checks,__LINE__,#x,errno);exit(1);}}while(0)
static void all(int fd,const void *bytes,size_t size) {
    const unsigned char *p=bytes;
    while(size){ssize_t n=write(fd,p,size);if(n<0 && errno==EINTR)continue;CHECK(n>0);p+=n;size-=(size_t)n;}
}
enum mode { EARLY, FRAGMENT, WRONG_UID, DEAD, LATE_OPTION, PARTIAL };
static void scenario(const char *root,enum mode mode,const uint8_t frame[LFG_FRAME_SIZE],const struct lfg_message *expected) {
    struct sockaddr_un address={.sun_family=AF_LOCAL};
    CHECK(snprintf(address.sun_path,sizeof(address.sun_path),"%s/sock-%d",root,mode)>0);
    address.sun_len=(uint8_t)(offsetof(struct sockaddr_un,sun_path)+strlen(address.sun_path)+1);
    int listener=socket(AF_LOCAL,SOCK_STREAM|SOCK_CLOEXEC,0);CHECK(listener>=0);
    if(mode!=LATE_OPTION)CHECK(!lfbg_enable_listener_credentials(listener));
    CHECK(!bind(listener,(struct sockaddr *)&address,address.sun_len));CHECK(!chmod(address.sun_path,0600));CHECK(!listen(listener,4));
    int done[2],release[2];CHECK(!pipe2(done,O_CLOEXEC) && !pipe2(release,O_CLOEXEC));
    pid_t child=fork();CHECK(child>=0);
    if(!child) {
        CHECK(!close(listener) && !close(done[0]) && !close(release[1]));
        int client_fd=socket(AF_LOCAL,SOCK_STREAM|SOCK_CLOEXEC,0);CHECK(client_fd>=0);
        CHECK(!connect(client_fd,(struct sockaddr *)&address,address.sun_len));
        if(mode==FRAGMENT){all(client_fd,frame,7);all(client_fd,frame+7,LFG_FRAME_SIZE-7);}
        else all(client_fd,frame,mode==PARTIAL?127:LFG_FRAME_SIZE);
        all(done[1],"s",1);CHECK(!close(done[1]));
        if(mode!=DEAD){char byte;CHECK(read(release[0],&byte,1)==1 && byte=='a');}
        CHECK(!close(release[0]) && !close(client_fd));_exit(0);
    }
    CHECK(!close(done[1]) && !close(release[0]));char byte;CHECK(read(done[0],&byte,1)==1 && byte=='s');CHECK(!close(done[0]));
    if(mode==DEAD)usleep(50000);
    int accepted=accept4(listener,NULL,NULL,SOCK_CLOEXEC|SOCK_NONBLOCK);CHECK(accepted>=0);
    if(mode==LATE_OPTION)CHECK(!lfbg_enable_credentials(accepted));
    struct lfbg_stream stream;struct lfg_message received,sentinel;
    memset(&sentinel,0x5a,sizeof(sentinel));received=sentinel;int64_t until;CHECK(!lfbg_deadline(mode==PARTIAL?50:1000,&until));
    uid_t uid=getuid();if(mode==WRONG_UID)++uid;
    int rc=lfbg_public_receive_first(&stream,accepted,uid,&received,until);
    if(mode==EARLY || mode==FRAGMENT) {
        CHECK(rc==1 && !memcmp(&received,expected,sizeof(received)));
        CHECK(stream.authenticated && !stream.discovering && stream.expected.pid==(uint64_t)child);
        CHECK(stream.expected.real_uid==getuid() && stream.expected.effective_uid==geteuid());
        CHECK(!lfbg_peer_current(&stream.expected,LFBG_PUBLIC_LIVE));
    } else {
        CHECK(rc<0 && !memcmp(&received,&sentinel,sizeof(received)));
        if(mode==WRONG_UID || mode==LATE_OPTION)CHECK(errno==EPERM);
        /* Native getpeereid can reject the disconnected socket before the
         * subsequent live PID check sees the unreaped zombie. Both refuse. */
        else if(mode==DEAD)CHECK(errno==ESRCH || errno==ENOTCONN);
        else CHECK(errno==ETIMEDOUT);
    }
    if(mode!=DEAD)all(release[1],"a",1);
    CHECK(!close(release[1]));int status;CHECK(waitpid(child,&status,0)==child && WIFEXITED(status) && !WEXITSTATUS(status));
    CHECK(!close(accepted) && !close(listener));
    struct stat socket;CHECK(!lstat(address.sun_path,&socket) && S_ISSOCK(socket.st_mode) && socket.st_uid==getuid());CHECK(!unlink(address.sun_path));
}
int main(int argc,char **argv) {
    CHECK(argc==3 && getuid()!=0 && getuid()==geteuid());
    struct stat dir;CHECK(!lstat(argv[1],&dir) && S_ISDIR(dir.st_mode) && dir.st_uid==getuid() && !(dir.st_mode&0077));
    FILE *f=fopen(argv[2],"r");CHECK(f!=NULL);uint8_t bytes[LFG_FRAME_SIZE];unsigned x;
    for(size_t i=0;i<sizeof(bytes);++i){CHECK(fscanf(f,"%2x",&x)==1);bytes[i]=(uint8_t)x;}
    CHECK(fscanf(f,"%2x",&x)==EOF && !fclose(f));struct lfg_message message;CHECK(lfg_decode(&message,bytes,sizeof(bytes))==LFG_OK);
    for(int mode=EARLY;mode<=PARTIAL;++mode)scenario(argv[1],(enum mode)mode,bytes,&message);
    printf("{\"result\":\"pass\",\"checks\":%u,\"cases\":6,\"scope\":\"native listener credential inheritance, early/fragmented public writes, live peer discovery, wrong UID/dead peer/late option/partial refusal; dummy children only\"}\n",checks);return 0;
}
