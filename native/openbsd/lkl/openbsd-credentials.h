/* SPDX-License-Identifier: GPL-2.0-only */
/* Reviewed BSD credential model, bound to one native callback thread. */
#ifndef LURKER_LKL_OPENBSD_CREDENTIALS_H
#define LURKER_LKL_OPENBSD_CREDENTIALS_H
#include "openbsd-caller.h"
#include <stdbool.h>
#include <pthread.h>
#include <lkl.h>
struct ob_credentials {
    pthread_t owner;
    bool initialized, active;
    int poison;
};
static int ob_group_order(const void *a,const void *b)
{
    uint32_t x=*(const uint32_t*)a,y=*(const uint32_t*)b;
    return (x>y)-(x<y);
}
static int ob_verify_groups(const struct ob_caller *caller)
{
    lkl_gid_t actual[KI_NGROUPS];uint32_t expected[KI_NGROUPS];
    long count=lkl_sys_getgroups(KI_NGROUPS,actual);
    if(count!=(long)caller->ngroups)return EIO;
    memcpy(expected,caller->groups,caller->ngroups*sizeof(*expected));
    qsort(expected,caller->ngroups,sizeof(*expected),ob_group_order);
    qsort(actual,caller->ngroups,sizeof(*actual),ob_group_order);
    return memcmp(expected,actual,caller->ngroups*sizeof(*expected))?EIO:0;
}
static int ob_owner(const struct ob_credentials *state)
{ return state->initialized && pthread_equal(state->owner,pthread_self()); }
static int ob_restore_credentials(struct ob_credentials *state)
{
    if(!ob_owner(state))return EPERM;
    long u=lkl_sys_setresuid(0,0,0),g=lkl_sys_setresgid(0,0,0);
    long s=lkl_sys_setgroups(0,NULL),m=lkl_sys_umask(0);
    lkl_uid_t r=(lkl_uid_t)-1,e=r,v=r;lkl_gid_t gr=(lkl_gid_t)-1,ge=gr,gv=gr;
    bool failed=u || g || s || m<0 || lkl_sys_getresuid(&r,&e,&v) ||
        lkl_sys_getresgid(&gr,&ge,&gv) || r || e || v || gr || ge || gv ||
        lkl_sys_setfsuid((lkl_uid_t)-1)!=0 || lkl_sys_setfsgid((lkl_gid_t)-1)!=0 ||
        lkl_sys_getgroups(0,NULL)!=0 || lkl_sys_umask(0)!=0;
    if(failed) {if(!state->poison)state->poison=EIO;state->active=true;return EIO;}
    state->active=false;return 0;
}
static int ob_credentials_init(struct ob_credentials *state)
{
    if(state->initialized)return EALREADY;
    memset(state,0,sizeof(*state));state->owner=pthread_self();state->initialized=true;
    return ob_restore_credentials(state);
}
static int ob_enter_credentials(struct ob_credentials *state,
                                const struct ob_caller *caller,unsigned mask)
{
    if(!ob_owner(state))return EPERM;
    if(state->poison)return EIO;
    if(state->active)return EDEADLK;
    if(mask&~0777U || caller->ngroups>KI_NGROUPS || !caller->ngroups ||
       caller->uid==(uid_t)-1 || caller->gid==(gid_t)-1)return EINVAL;
    state->active=true;
    lkl_gid_t groups[KI_NGROUPS];
    for(unsigned i=0;i<caller->ngroups;i++)groups[i]=caller->groups[i];
    long ret=lkl_sys_setgroups(caller->ngroups,groups);
    if(!ret)ret=lkl_sys_setresgid(caller->gid,caller->gid,0);
    if(!ret)ret=lkl_sys_setresuid(caller->uid,caller->uid,0);
    if(!ret && lkl_sys_umask(mask)<0)ret=-LKL_EIO;
    lkl_uid_t r,e,s;lkl_gid_t gr,ge,gs;
    if(!ret && (lkl_sys_getresuid(&r,&e,&s) || lkl_sys_getresgid(&gr,&ge,&gs) ||
        r!=caller->uid || e!=caller->uid || s || gr!=caller->gid || ge!=caller->gid || gs ||
        lkl_sys_setfsuid((lkl_uid_t)-1)!=caller->uid ||
        lkl_sys_setfsgid((lkl_gid_t)-1)!=caller->gid || ob_verify_groups(caller) ||
        lkl_sys_umask(mask)!=(long)mask))ret=-LKL_EIO;
    if(ret<0){(void)ob_restore_credentials(state);return EIO;}
    return 0;
}
#endif
