#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#define FUSE_USE_VERSION 26
#include <fuse.h>
#include <fuse_lowlevel.h>
extern struct fuse_chan *fuse_kern_chan_new(int fd);
#include "../../lkl/dragonfly-channel.h"
int main(int argc, char **argv)
{
    assert(getuid()==1300 && geteuid()==1300 && argc==1);
    int fd=open("/dev/null",O_RDWR|O_CLOEXEC);assert(fd>=0);
    struct fuse_chan *inner=fuse_kern_chan_new(fd);assert(inner);
    struct fuse_chan *outer=df_observed_channel(inner);assert(outer);
    struct fuse_args args=FUSE_ARGS_INIT(argc,argv);struct fuse_operations ops={0};
    struct fuse *fuse=fuse_new(outer,&args,&ops,sizeof(ops),NULL);assert(fuse);
    struct fuse_session *session=fuse_get_session(fuse);
    char buffer[4096];struct fuse_chan *received=outer;
    assert(fuse_chan_recv(&received,buffer,sizeof(buffer))==-EIO);
    assert(df_transport_error && received==outer && fuse_chan_session(outer)==session && !fuse_chan_session(inner));
    struct fuse_out_header header={.len=sizeof(header),.error=0,.unique=1};
    struct iovec iov={.iov_base=&header,.iov_len=sizeof(header)};
    assert(fuse_chan_send(outer,&iov,1)==0);
    assert(fuse_chan_session(outer)==session && !fuse_chan_session(inner));
    fuse_opt_free_args(&args);fuse_destroy(fuse);errno=0;
    assert(fcntl(fd,F_GETFD)==-1 && errno==EBADF);
    puts("native delegated receive-error/send-success preserve sole session attachment and dispose inner descriptor; no mounted runtime credit");return 0;
}
