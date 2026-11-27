/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef LKLFUSE_NETBSD_OBSERVER_H
#define LKLFUSE_NETBSD_OBSERVER_H
#include "netbsd-mount.h"
#include <sys/wait.h>
#include <fcntl.h>
#include <signal.h>
#include <spawn.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>

#define NETBSD_MOUNT_WIRE_MAGIC 0x4e424d31U
struct netbsd_mount_wire { uint32_t magic, record_bytes, count; };
struct netbsd_observer {
    pid_t pid;
    int fd,error,status;
    bool active,eof,stopping,killed,stuck;
    uint64_t deadline,kill_at,stuck_at;
    unsigned char *buffer;
    size_t bytes,capacity;
    struct netbsd_mounts result;
};

static inline uint64_t netbsd_observer_now(void)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC,&now) || now.tv_sec<0 ||
        (uint64_t)now.tv_sec>UINT64_MAX/1000000000ULL) return 0;
    return (uint64_t)now.tv_sec*1000000000ULL+now.tv_nsec;
}

static inline void netbsd_observer_cancel(struct netbsd_observer *job,int error)
{
    if (!job->error) job->error=error>0?error:EIO;
    if (!job->active || job->stopping) return;
    job->stopping=true;
    uint64_t now=netbsd_observer_now();
    job->kill_at=now?now+200000000ULL:0;
    /* The unreaped child is its process-group leader and reserves the PGID. */
    if (kill(-job->pid,SIGTERM) && errno!=ESRCH) job->error=EIO;
}

static inline int netbsd_observer_start(struct netbsd_observer *job,
    char *const argv[],char *const environment[],unsigned int timeout_ms)
{
    int fd[2],error;
    uint64_t now=netbsd_observer_now();
    if (job->active || job->buffer || job->result.items) return EBUSY;
    if (!argv || !argv[0] || argv[0][0]!='/' || !timeout_ms || timeout_ms>30000 || !now)
        return EINVAL;
    *job=(struct netbsd_observer){.pid=-1,.fd=-1};
    if (pipe2(fd,O_CLOEXEC)) return errno;
    if (fd[0]<=STDERR_FILENO || fd[1]<=STDERR_FILENO) {
        close(fd[0]);close(fd[1]);return EINVAL;
    }
    if (fcntl(fd[0],F_SETFL,O_NONBLOCK)) {
        error=errno;close(fd[0]);close(fd[1]);return error;
    }
    job->capacity=sizeof(struct netbsd_mount_wire)+4096*sizeof(struct statvfs);
    job->buffer=malloc(job->capacity);
    if (!job->buffer) {close(fd[0]);close(fd[1]);return ENOMEM;}
    posix_spawn_file_actions_t actions;
    posix_spawnattr_t attributes;
    bool have_actions=false,have_attributes=false;
    if ((error=posix_spawn_file_actions_init(&actions))) goto failed;
    have_actions=true;
    if ((error=posix_spawnattr_init(&attributes))) goto failed;
    have_attributes=true;
    if ((error=posix_spawn_file_actions_adddup2(&actions,fd[1],STDOUT_FILENO)) ||
        (error=posix_spawn_file_actions_addclose(&actions,fd[0])) ||
        (error=posix_spawn_file_actions_addclose(&actions,fd[1])) ||
        (error=posix_spawnattr_setpgroup(&attributes,0)) ||
        (error=posix_spawnattr_setflags(&attributes,POSIX_SPAWN_SETPGROUP))) goto failed;
    error=posix_spawn(&job->pid,argv[0],&actions,&attributes,argv,environment);
failed:
    if (have_attributes) posix_spawnattr_destroy(&attributes);
    if (have_actions) posix_spawn_file_actions_destroy(&actions);
    close(fd[1]);
    if (error) {
        close(fd[0]);free(job->buffer);job->buffer=NULL;return error;
    }
    job->fd=fd[0];job->active=true;
    job->deadline=now+(uint64_t)timeout_ms*1000000ULL;
    return 0;
}

static inline void netbsd_observer_parse(struct netbsd_observer *job)
{
    struct netbsd_mount_wire header;
    if (job->bytes<sizeof(header)) {job->error=EIO;return;}
    memcpy(&header,job->buffer,sizeof(header));
    if (header.magic!=NETBSD_MOUNT_WIRE_MAGIC || header.record_bytes!=sizeof(struct statvfs) ||
        !header.count || header.count>4096 ||
        job->bytes!=sizeof(header)+(size_t)header.count*sizeof(struct statvfs)) {
        job->error=EIO;return;
    }
    struct statvfs *items=malloc((size_t)header.count*sizeof(*items));
    if (!items) {job->error=ENOMEM;return;}
    memcpy(items,job->buffer+sizeof(header),(size_t)header.count*sizeof(*items));
    job->result=(struct netbsd_mounts){.items=items,.count=header.count};
}

/* Nonblocking native-only tick. No emulated syscall or mount operation here. */
static inline void netbsd_observer_tick(struct netbsd_observer *job)
{
    if (!job->active) return;
    uint64_t now=netbsd_observer_now();
    if (!now) netbsd_observer_cancel(job,EIO);
    if (!job->stopping && now>=job->deadline) netbsd_observer_cancel(job,ETIMEDOUT);
    for (;;) {
        unsigned char extra;
        size_t room=job->capacity-job->bytes;
        ssize_t count=read(job->fd,room?job->buffer+job->bytes:&extra,room?room:1);
        if (count>0) {
            if (!room) {netbsd_observer_cancel(job,EOVERFLOW);break;}
            job->bytes+=count;
            continue;
        }
        if (!count) job->eof=true;
        else if (errno!=EAGAIN && errno!=EINTR) netbsd_observer_cancel(job,errno);
        break;
    }
    if (job->stopping && !job->killed) {
        if (now<job->kill_at) return;
        /* Never reap/poll between TERM and unconditional group KILL. */
        if (kill(-job->pid,SIGKILL) && errno!=ESRCH) job->error=EIO;
        job->killed=true;job->stuck_at=now+1000000000ULL;
    }
    if (!job->stopping && !job->eof) return;
    pid_t done=waitpid(job->pid,&job->status,WNOHANG);
    if (done==0 || (done<0 && errno==EINTR)) {
        if (job->killed && now>=job->stuck_at) job->stuck=true;
        return; /* Keep ownership; never start another worker or infer absence. */
    }
    if (done<0 || !WIFEXITED(job->status) || WEXITSTATUS(job->status)) {
        if (!job->error) job->error=EIO;
    }
    if (!job->error) netbsd_observer_parse(job);
    close(job->fd);job->fd=-1;job->active=false;
    free(job->buffer);job->buffer=NULL;
}

static inline int netbsd_observer_take(struct netbsd_observer *job,
                                       struct netbsd_mounts *result)
{
    if (job->active) return EAGAIN;
    if (job->error) return job->error;
    if (!job->result.items) return ENOENT;
    *result=job->result;job->result=(struct netbsd_mounts){0};
    return 0;
}
#endif
