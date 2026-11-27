/* SPDX-License-Identifier: MIT */
#include "../../../lkl/guardian-protocol.h"
#include "../guardian-platform.h"
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
static unsigned checks;
#define CHECK(x) do { ++checks; if (!(x)) { \
 fprintf(stderr,"check %u line %u failed: %s (errno %d)\n",checks,__LINE__,#x,errno);exit(1); } } while(0)
int main(void) {
    struct lfbg_process before, after, sentinel;
    CHECK(getuid() != 0 && geteuid() == getuid());
    CHECK(lfbg_process_capture(getpid(), &before) == 0);
    CHECK(before.pid == (uint64_t)getpid() && before.real_uid == getuid() &&
          before.effective_uid == geteuid());
    CHECK(lfbg_process_capture(getpid(), &after) == 0);
    CHECK(lfbg_process_same(&before, &after));
    uint8_t boot[16];lfbg_process_boot_bytes(&before, boot);
    CHECK(lfg_get64(boot) == before.boot_sec && lfg_get64(boot + 8) == before.boot_usec);
    sentinel = before;
    CHECK(lfbg_process_capture(0, &sentinel) < 0);
    CHECK(lfbg_process_same(&sentinel, &before));
    pid_t child = fork();CHECK(child >= 0);
    if (!child) _exit(0);
    int status;CHECK(waitpid(child,&status,0) == child && WIFEXITED(status));
    CHECK(lfbg_process_capture(child,&sentinel) < 0);
    CHECK(lfbg_process_same(&sentinel,&before));

    char path[] = "/tmp/lkl-guardian-source-XXXXXX";
    int fd = mkstemp(path);CHECK(fd >= 0);
    CHECK(fchmod(fd,0600) == 0 && ftruncate(fd,4096) == 0);
    struct lfbg_source source, changed, copy;
    CHECK(lfbg_source_capture(fd,&source) == 0);
    CHECK(source.kind == LFG_SOURCE_REGULAR && source.bytes == 4096 &&
          source.rdev == 0 && source.logical_sector == 0 && source.physical_sector == 0);
    CHECK(lfbg_source_flush(fd,&source) == 0);
    changed = source;changed.inode++;
    CHECK(lfbg_source_flush(fd,&changed) < 0 && errno == ESTALE);
    CHECK(fcntl(fd,F_GETFD) >= 0);
    CHECK(ftruncate(fd,8192) == 0);
    CHECK(lfbg_source_flush(fd,&source) < 0 && errno == ESTALE);
    CHECK(lfbg_source_capture(fd,&changed) == 0 && !lfbg_source_same(&source,&changed));
    CHECK(lfbg_source_close(&fd) == 0 && fd == -1);
    CHECK(lfbg_source_close(&fd) < 0 && errno == EBADF);
    CHECK(unlink(path) == 0);
    int non_disk = open("/dev/null",O_RDONLY|O_CLOEXEC);CHECK(non_disk >= 0);
    copy = source;
    CHECK(lfbg_source_capture(non_disk,&copy) < 0);
    CHECK(lfbg_source_same(&copy,&source));
    CHECK(close(non_disk) == 0);
    printf("{\"checks\":%u,\"result\":\"pass\",\"scope\":\"native identity and regular held-source helpers only; no guardian authorization or mounts\"}\n",checks);
    return 0;
}
