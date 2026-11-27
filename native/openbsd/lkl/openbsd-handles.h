/* SPDX-License-Identifier: GPL-2.0-only */
/* Tokens grant only registered descriptor cleanup, never ambient-root paths. */
#ifndef LURKER_LKL_OPENBSD_HANDLES_H
#define LURKER_LKL_OPENBSD_HANDLES_H
#include "openbsd-credentials.h"
enum ob_handle_kind {OB_FILE=1,OB_DIRECTORY=2};
struct ob_handle {uint64_t token;long fd;enum ob_handle_kind kind;int flags;struct ob_caller opener;};
struct ob_handles {struct ob_handle slots[128];uint64_t next;int failure;};
static struct ob_handle *ob_find_handle(struct ob_handles *registry,uint64_t token,enum ob_handle_kind kind)
{
    if(!token)return NULL;
    for(unsigned i=0;i<128;i++)if(registry->slots[i].token==token && registry->slots[i].kind==kind)return &registry->slots[i];
    return NULL;
}
static int ob_register_handle(struct ob_handles *registry,long fd,int flags,
                              enum ob_handle_kind kind,const struct ob_caller *caller,uint64_t *token)
{
    if(fd<0 || (kind!=OB_FILE && kind!=OB_DIRECTORY))return EINVAL;
    if(!registry->next || registry->next==UINT64_MAX)return EOVERFLOW;
    for(unsigned i=0;i<128;i++)if(registry->slots[i].token && registry->slots[i].fd==fd)return EEXIST;
    for(unsigned i=0;i<128;i++)if(!registry->slots[i].token){
        registry->slots[i]=(struct ob_handle){.token=registry->next++,.fd=fd,.kind=kind,.flags=flags,.opener=*caller};
        *token=registry->slots[i].token;return 0;
    }
    return EMFILE;
}
static int ob_handle_barrier(struct ob_handles *registry,uint64_t token,enum ob_handle_kind kind)
{
    struct ob_handle *h=ob_find_handle(registry,token,kind);if(!h)return EBADF;
    long ret=lkl_sys_fsync(h->fd);
    if(ret<0){if(!registry->failure)registry->failure=EIO;return EIO;}
    return 0;
}
static int ob_release_handle(struct ob_handles *registry,uint64_t token,enum ob_handle_kind kind)
{
    struct ob_handle *h=ob_find_handle(registry,token,kind);if(!h)return EBADF;
    long fd=h->fd;memset(h,0,sizeof(*h));
    /* Consume once even after an ambiguous close error; never close a reused FD. */
    long ret=lkl_sys_close(fd);
    if(ret<0){if(!registry->failure)registry->failure=EIO;return EIO;}
    return 0;
}
#endif
