/* SPDX-License-Identifier: GPL-2.0-only */
/* Release gate only. Exact global native FSID observation remains a frontend gate. */
#ifndef LURKER_LKL_OPENBSD_SHUTDOWN_H
#define LURKER_LKL_OPENBSD_SHUTDOWN_H
#include <stdbool.h>
#include <errno.h>
struct ob_shutdown {
    bool published,captured,absent,drained,kernel_started,halted;
    bool channel_open,source_open,cleaned;
    int failure;
};
static void ob_shutdown_failure(struct ob_shutdown *s,int error)
{if(!s->failure)s->failure=error>0?error:EIO;}
static int ob_shutdown_observe(struct ob_shutdown *s,int present)
{
    if(!s->published || !s->captured)return EINVAL;
    s->absent=false;
    if(present<0){s->drained=false;ob_shutdown_failure(s,EIO);return EIO;}
    if(present){s->drained=false;return EBUSY;}
    s->absent=true;return 0;
}
static int ob_shutdown_drain(struct ob_shutdown *s,long result)
{
    if(s->published && (!s->captured || !s->absent))return EBUSY;
    if(result){ob_shutdown_failure(s,EIO);return EIO;}
    s->drained=true;return 0;
}
static int ob_shutdown_halt(struct ob_shutdown *s,long(*halt)(void))
{
    if(s->published && (!s->captured || !s->absent))return EBUSY;
    if(!s->kernel_started || s->halted)return 0;
    if(!s->drained)return EBUSY;
    if(halt()){ob_shutdown_failure(s,EIO);return EIO;}
    s->halted=true;return 0;
}
struct ob_shutdown_ops {int(*barrier)(void*);int(*channel)(void*);int(*source)(void*);void(*cleanup)(void*);};
static int ob_shutdown_release(struct ob_shutdown *s,const struct ob_shutdown_ops *ops,void *arg)
{
    if((s->published && (!s->captured || !s->absent)) || (s->kernel_started && !s->halted))return EBUSY;
    if(s->cleaned)return s->failure;
    int error;
    if(s->source_open && (error=ops->barrier(arg))){ob_shutdown_failure(s,error);return error;}
    if(s->channel_open){if((error=ops->channel(arg))){ob_shutdown_failure(s,error);return error;}s->channel_open=false;}
    if(s->source_open){if((error=ops->source(arg))){ob_shutdown_failure(s,error);return error;}s->source_open=false;}
    ops->cleanup(arg);s->cleaned=true;return s->failure;
}
#endif
