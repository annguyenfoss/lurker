/* SPDX-License-Identifier: MIT
 * Native user-only stream/SCM_CREDS2 controls. Dummy owned children and sockets;
 * no LKL, FUSE, mount, ioctl, encryption or production admission.
 */
#include "../../../lkl/guardian-protocol.h"
#include "../guardian-platform.h"
#include <stdio.h>
#include <sys/wait.h>

static unsigned checks;
#define CHECK(x) do { ++checks; if (!(x)) { \
    fprintf(stderr,"check %u line %u failed: %s errno=%d\n",checks,__LINE__,#x,errno);exit(1); \
} } while (0)

static struct lfg_message example(const char *path) {
    FILE *f=fopen(path,"r"); uint8_t bytes[LFG_FRAME_SIZE]; unsigned x;
    CHECK(f!=NULL);
    for (size_t i=0;i<sizeof(bytes);++i) { CHECK(fscanf(f,"%2x",&x)==1); bytes[i]=(uint8_t)x; }
    CHECK(fscanf(f,"%2x",&x)==EOF && !fclose(f));
    struct lfg_message m; CHECK(lfg_decode(&m,bytes,sizeof(bytes))==LFG_OK); return m;
}
static void write_all(int fd,const void *data,size_t size) {
    const unsigned char *p=data;
    while (size) { ssize_t n=write(fd,p,size); if(n<0 && errno==EINTR)continue; CHECK(n>0);p+=n;size-=(size_t)n; }
}
static void await_byte(int fd) { char byte;CHECK(read(fd,&byte,1)==1 && byte=='a'); }
static void finish(pid_t child,int control) {
    write_all(control,"a",1);CHECK(!close(control)); int status;
    CHECK(waitpid(child,&status,0)==child && WIFEXITED(status) && !WEXITSTATUS(status));
}
enum scenario { FRAGMENTED, QUEUED_ZOMBIE, PARTIAL, PARTIAL_EOF, MALFORMED, RIGHTS, OTHER_WRITER, QUIET, ECHO };
struct fixture { pid_t child;int socket,control;struct lfbg_process peer;struct lfbg_stream stream; };

static struct fixture start(enum scenario mode,const struct lfg_message *message) {
    int pair[2],control[2];CHECK(!socketpair(AF_LOCAL,SOCK_STREAM|SOCK_CLOEXEC,0,pair));
    CHECK(!lfbg_enable_credentials(pair[0]) && !lfbg_enable_credentials(pair[1]));
    CHECK(!pipe2(control,O_CLOEXEC));pid_t child=fork();CHECK(child>=0);
    if(!child) {
        close(pair[0]);close(control[1]);await_byte(control[0]);
        uint8_t bytes[LFG_FRAME_SIZE];CHECK(lfg_encode(bytes,sizeof(bytes),message)==LFG_OK);
        if(mode==FRAGMENTED || mode==QUEUED_ZOMBIE) {
            write_all(pair[1],bytes,7);write_all(pair[1],bytes+7,123);
            write_all(pair[1],bytes+130,sizeof(bytes)-130);
            if(mode==QUEUED_ZOMBIE) write_all(pair[1],bytes,sizeof(bytes));
        } else if(mode==PARTIAL || mode==PARTIAL_EOF) write_all(pair[1],bytes,127);
        else if(mode==MALFORMED) {bytes[0]^=1;write_all(pair[1],bytes,sizeof(bytes));}
        else if(mode==RIGHTS) {
            int fd=open("/dev/null",O_RDONLY|O_CLOEXEC);CHECK(fd>=0);
            union {struct cmsghdr align;unsigned char bytes[CMSG_SPACE(sizeof(int))];} ancillary;
            memset(&ancillary,0,sizeof(ancillary));struct iovec io={bytes,sizeof(bytes)};
            struct msghdr msg={.msg_iov=&io,.msg_iovlen=1,.msg_control=ancillary.bytes,.msg_controllen=sizeof(ancillary.bytes)};
            struct cmsghdr *c=CMSG_FIRSTHDR(&msg);c->cmsg_level=SOL_SOCKET;c->cmsg_type=SCM_RIGHTS;c->cmsg_len=CMSG_LEN(sizeof(int));
            memcpy(CMSG_DATA(c),&fd,sizeof(fd));CHECK(sendmsg(pair[1],&msg,MSG_NOSIGNAL)==sizeof(bytes));CHECK(!close(fd));
        } else if(mode==OTHER_WRITER) {
            pid_t grandchild=fork();CHECK(grandchild>=0);
            if(!grandchild){write_all(pair[1],bytes,sizeof(bytes));_exit(0);}
            int status;CHECK(waitpid(grandchild,&status,0)==grandchild && WIFEXITED(status) && !WEXITSTATUS(status));
        } else if(mode==ECHO) {
            struct lfbg_process parent;struct lfbg_stream stream;struct lfg_message received;int64_t until;
            CHECK(!lfbg_process_capture(getppid(),&parent));
            CHECK(!lfbg_stream_init(&stream,pair[1],&parent,LFBG_PUBLIC_LIVE));
            CHECK(!lfbg_deadline(1000,&until));
            CHECK(lfbg_receive_until(&stream,&received,until)==1 && !memcmp(&received,message,sizeof(received)));
            CHECK(!lfbg_send_until(&stream,&received,until));
        }
        if(mode!=QUEUED_ZOMBIE && mode!=PARTIAL_EOF) await_byte(control[0]);
        CHECK(!close(pair[1]) && !close(control[0]));_exit(0);
    }
    CHECK(!close(pair[1]) && !close(control[0]));
    struct fixture f={.child=child,.socket=pair[0],.control=control[1]};
    CHECK(!lfbg_process_capture(child,&f.peer));
    CHECK(!lfbg_stream_init(&f.stream,f.socket,&f.peer,LFBG_OWNED_PROGRESS));
    write_all(f.control,"a",1);return f;
}
static int descriptor_count(void) {int n=0;for(int fd=0;fd<256;++fd)if(fcntl(fd,F_GETFD)>=0)++n;return n;}
static void rpc_controls(struct lfg_message identity) {
    struct lfg_message request,second,reply;
    CHECK(!lfbg_rpc_request(&identity,LFG_QUERY,&request));
    CHECK(!lfbg_rpc_request(&identity,LFG_QUERY,&second));
    CHECK(memcmp(request.request_nonce,second.request_nonce,sizeof(request.request_nonce)));
    reply=identity;memcpy(reply.request_nonce,request.request_nonce,sizeof(reply.request_nonce));
    CHECK(!lfbg_rpc_match(&identity,&request,&reply));
    CHECK(lfbg_rpc_match(&identity,&second,&reply)<0 && errno==ESTALE);
    second=request;++second.native_id[0];CHECK(lfbg_rpc_match(&identity,&second,&reply)<0 && errno==ESTALE);
    second=reply;++second.source_ino;CHECK(lfbg_rpc_match(&identity,&request,&second)<0 && errno==ESTALE);
    second=reply;++second.native_id[0];CHECK(lfbg_rpc_match(&identity,&request,&second)<0 && errno==ESTALE);
    second=request;second.kind=LFG_READY;CHECK(lfbg_rpc_match(&identity,&second,&reply)<0);
    CHECK(lfbg_rpc_request(&identity,LFG_READY,&second)<0 && errno==EINVAL);
    memset(identity.native_id,0,sizeof(identity.native_id));
    CHECK(!lfbg_rpc_request(&identity,LFG_QUERY,&request));reply=identity;
    memcpy(reply.request_nonce,request.request_nonce,sizeof(reply.request_nonce));
    CHECK(lfbg_rpc_match(&identity,&request,&reply)<0 && errno==ESTALE);
}
int main(int argc,char **argv) {
    CHECK(argc==2 && getuid()!=0 && getuid()==geteuid());
    struct lfg_message m=example(argv[1]),out,sentinel;rpc_controls(m);
    int64_t deadline;CHECK(lfbg_deadline(0,&deadline)<0 && errno==EINVAL);
    CHECK(lfbg_deadline(30001,&deadline)<0 && errno==EINVAL);
    struct fixture f=start(FRAGMENTED,&m);CHECK(!lfbg_deadline(1000,&deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)==1 && !memcmp(&out,&m,sizeof(m)));
    struct lfbg_process bad=f.peer;++bad.birth_usec;
    struct lfbg_stream invalid;CHECK(lfbg_stream_init(&invalid,f.socket,&bad,LFBG_OWNED_PROGRESS)<0 && errno==ESTALE);
    CHECK(lfbg_stream_init(&invalid,f.socket,&f.peer,(enum lfbg_peer_kind)99)<0 && errno==EINVAL);
    finish(f.child,f.control);CHECK(!close(f.socket));

    f=start(ECHO,&m);CHECK(!lfbg_deadline(1000,&deadline));
    CHECK(!lfbg_send_until(&f.stream,&m,deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)==1 && !memcmp(&out,&m,sizeof(m)));
    finish(f.child,f.control);CHECK(!close(f.socket));

    f=start(QUEUED_ZOMBIE,&m);usleep(50000);
    CHECK(!lfbg_peer_current(&f.peer,LFBG_OWNED_PROGRESS));
    CHECK(lfbg_peer_current(&f.peer,LFBG_PUBLIC_LIVE)<0 && errno==ESRCH);
    CHECK(!lfbg_deadline(1000,&deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)==1 && !memcmp(&out,&m,sizeof(m)));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)==1 && !memcmp(&out,&m,sizeof(m)));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)==0);
    int status;CHECK(waitpid(f.child,&status,0)==f.child && WIFEXITED(status) && !WEXITSTATUS(status));
    CHECK(lfbg_peer_current(&f.peer,LFBG_OWNED_PROGRESS)<0 && errno==ESRCH);
    CHECK(!close(f.control) && !close(f.socket));

    f=start(PARTIAL,&m);memset(&sentinel,0x5a,sizeof(sentinel));out=sentinel;
    CHECK(!lfbg_deadline(80,&deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)<0 && errno==ETIMEDOUT && !memcmp(&out,&sentinel,sizeof(out)));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)<0 && errno==ETIMEDOUT);
    finish(f.child,f.control);CHECK(!close(f.socket));

    f=start(PARTIAL_EOF,&m);out=sentinel;CHECK(!lfbg_deadline(1000,&deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)<0 && errno==EPROTO && !memcmp(&out,&sentinel,sizeof(out)));
    CHECK(waitpid(f.child,&status,0)==f.child && WIFEXITED(status) && !WEXITSTATUS(status));
    CHECK(!close(f.control) && !close(f.socket));

    f=start(MALFORMED,&m);out=sentinel;CHECK(!lfbg_deadline(1000,&deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)<0 && errno==EPROTO && !memcmp(&out,&sentinel,sizeof(out)));
    finish(f.child,f.control);CHECK(!close(f.socket));

    f=start(RIGHTS,&m);int before=descriptor_count();CHECK(!lfbg_deadline(1000,&deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)<0 && errno==EPROTO);
    CHECK(descriptor_count()==before);finish(f.child,f.control);CHECK(!close(f.socket));

    f=start(OTHER_WRITER,&m);CHECK(!lfbg_deadline(1000,&deadline));
    CHECK(lfbg_receive_until(&f.stream,&out,deadline)<0 && errno==EPERM);
    finish(f.child,f.control);CHECK(!close(f.socket));

    f=start(QUIET,&m);struct lfbg_stream public_stream;
    CHECK(!lfbg_stream_init(&public_stream,f.socket,&f.peer,LFBG_PUBLIC_LIVE));
    CHECK(!lfbg_deadline(50,&deadline));CHECK(lfbg_receive_until(&public_stream,&out,deadline)<0 && errno==ETIMEDOUT);
    finish(f.child,f.control);CHECK(!close(f.socket));
    printf("{\"result\":\"pass\",\"checks\":%u,\"cases\":10,\"scope\":\"native SCM_CREDS2 framing, live public peer, unreaped owned progress, deadlines, rights rejection and fresh RPC binding; dummy children only\"}\n",checks);
    return 0;
}
