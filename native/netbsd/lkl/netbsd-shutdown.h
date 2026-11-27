/* SPDX-License-Identifier: GPL-2.0-only */
/* A release gate, not a mount detector. The frontend must supply identity proof. */
#ifndef LKLFUSE_NETBSD_SHUTDOWN_H
#define LKLFUSE_NETBSD_SHUTDOWN_H
#include <stdbool.h>
#include <errno.h>

struct netbsd_shutdown {
    bool published, captured, absent;
    bool kernel_started, halted;
    bool channel_open, source_open, cleaned;
    int failure;
};

static void netbsd_shutdown_failure(struct netbsd_shutdown *state, int error)
{
    if (!state->failure) state->failure=error>0?error:EIO;
}

/* The caller must query the captured identity globally, including moved mounts.
 * A pathname replacement or a failed inventory query is never absence. */
static int netbsd_shutdown_observe(struct netbsd_shutdown *state, int present)
{
    if (!state->published || !state->captured) return EINVAL;
    state->absent=false;
    if (present<0) {
        netbsd_shutdown_failure(state,EIO);
        return EIO;
    }
    if (present) return EBUSY;
    state->absent=true;
    return 0;
}

static int netbsd_shutdown_halt(struct netbsd_shutdown *state, long (*halt)(void))
{
    if (state->published && (!state->captured || !state->absent)) return EBUSY;
    if (!state->kernel_started || state->halted) return 0;
    if (halt()!=0) {
        netbsd_shutdown_failure(state,EIO);
        /* Current LKL returns before CPU/timer/thread drain on failure. */
        return EIO;
    }
    state->halted=true;
    return 0;
}

struct netbsd_shutdown_ops {
    int (*final_barrier)(void *);
    int (*close_channel)(void *);
    int (*close_source)(void *);
    void (*cleanup)(void *);
};

static int netbsd_shutdown_release(struct netbsd_shutdown *state,
                                  const struct netbsd_shutdown_ops *ops, void *arg)
{
    int error;
    if ((state->published && (!state->captured || !state->absent)) ||
        (state->kernel_started && !state->halted)) return EBUSY;
    if (state->cleaned) return state->failure;
    if (state->source_open) {
        error=ops->final_barrier(arg);
        if (error) netbsd_shutdown_failure(state,error);
    }
    if (state->channel_open) {
        error=ops->close_channel(arg);
        if (error) {netbsd_shutdown_failure(state,error);return error;}
        state->channel_open=false;
    }
    if (state->source_open) {
        error=ops->close_source(arg);
        if (error) {netbsd_shutdown_failure(state,error);return error;}
        state->source_open=false;
    }
    ops->cleanup(arg);
    state->cleaned=true;
    return state->failure;
}
#endif
