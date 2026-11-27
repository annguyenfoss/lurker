/* SPDX-License-Identifier: GPL-2.0-only */
/* This executable always runs permanently as UID1300, never as root. */
#include "openbsd-caller.h"
#include <assert.h>
#include <dirent.h>
#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>
static int has(const struct ob_caller *c,gid_t group)
{for(unsigned i=0;i<c->ngroups;i++)if(c->groups[i]==group)return 1;return 0;}
int main(int argc,char **argv)
{
    assert(argc==5 && getuid()==1300 && geteuid()==1300);
    pid_t tid=strtol(argv[1],NULL,10);int command=atoi(argv[2]),ack=atoi(argv[3]);
    struct ob_caller before,after;
    assert(!ob_stable_caller(tid,1300,1300,&before) && has(&before,0));
    DIR *dir=opendir(argv[4]);assert(dir);assert(!closedir(dir));
    int dfd=open(argv[4],O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);assert(dfd>=0);
    int fd=openat(dfd,"group-only",O_RDWR|O_NOFOLLOW|O_CLOEXEC);assert(fd>=0);
    assert(pwrite(fd,"C",1,0)==1);char c;assert(pread(fd,&c,1,0)==1 && c=='C');assert(!close(fd));assert(!close(dfd));
    assert(write(command,"G",1)==1);assert(read(ack,&c,1)==1 && c=='G');
    assert(!ob_stable_caller(tid,1300,1300,&after));
    assert(before.pid==after.pid && before.thrid==after.thrid && before.birth_sec==after.birth_sec && before.birth_usec==after.birth_usec);
    assert(!ob_same_caller(&before,&after) && !has(&after,0) && has(&after,32766));
    assert(write(command,"Q",1)==1);assert(!close(command));assert(!close(ack));
    puts("native live same-birth THRID group change detected with unchanged wire UID/GID; actual UID1300 secondary-group-only list/read/write passed, changed-group actor and UID32767 denied; wire timing limit remains explicit");
}
