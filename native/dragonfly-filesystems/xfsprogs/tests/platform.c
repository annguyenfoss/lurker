#include <xfs/libxfs.h>
#include <sys/wait.h>

char *progname = "DragonFly formatter platform control";
extern void platform_findsizes(char *, int, long long *, int *);
extern void platform_flush_device(int, dev_t);

int main(int argc, char **argv)
{
    assert(argc == 2 && getuid() == 1300 && geteuid() == 1300);
    int fd = open(argv[1], O_CREAT | O_EXCL | O_RDWR, 0600);
    assert(fd >= 0);
    assert(ftruncate(fd, 512LL * 1024 * 1024) == 0);
    long long size = 0;
    int block = 0;
    platform_findsizes(argv[1], fd, &size, &block);
    assert(size == 1048576 && block == 512);
    platform_flush_device(fd, 0);
    assert(close(fd) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        platform_flush_device(-1, 0);
        _exit(99);
    }
    int status;
    assert(waitpid(child, &status, 0) == child);
    assert(WIFEXITED(status) && WEXITSTATUS(status) == 1);
    puts("regular-file geometry and native fsync error forwarding passed");
    return 0;
}
