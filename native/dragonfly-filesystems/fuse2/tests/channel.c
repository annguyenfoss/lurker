#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>
#define FUSE_USE_VERSION 26
#include <fuse.h>
#include <fuse_lowlevel.h>
/* Exact installed fuse_lowlevel_compat.h declaration and exported FUSE2 ABI;
 * that header explicitly prohibits direct inclusion at API26. */
extern struct fuse_chan *fuse_kern_chan_new(int fd);
int main(int argc, char **argv)
{
    assert(getuid() == 1300 && geteuid() == 1300 && argc == 1);
    int fd = open("/dev/null", O_RDWR | O_CLOEXEC);
    assert(fd >= 0);
    struct fuse_chan *channel = fuse_kern_chan_new(fd);
    assert(channel && fuse_chan_fd(channel) == fd);
    struct fuse_args args = FUSE_ARGS_INIT(argc, argv);
    struct fuse_operations ops = {0};
    struct fuse *fuse = fuse_new(channel, &args, &ops, sizeof(ops), NULL);
    assert(fuse && fuse_chan_session(channel) == fuse_get_session(fuse));
    fuse_opt_free_args(&args);
    fuse_destroy(fuse);
    errno = 0;
    assert(fcntl(fd, F_GETFD) == -1 && errno == EBADF);
    puts("native exported unmounted channel constructor + fuse_new session attachment + descriptor disposal passed; no mount/unmount called");
    return 0;
}
