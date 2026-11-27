#define _NETBSD_SOURCE
#include <sys/types.h>
#include <sys/vnode.h>
#include <fs/puffs/puffs_msgif.h>
#include <puffs.h>
#include <puffs_priv.h>
struct puffs_cc *bridge_test_getcc(struct puffs_usermount *);
int bridge_test_getcaller(struct puffs_cc *,pid_t *,lwpid_t *);
int bridge_test_mount(struct puffs_usermount *,const char *,int,puffs_cookie_t);
#define puffs_cc_getcc bridge_test_getcc
#define puffs_cc_getcaller bridge_test_getcaller
#define puffs_mount bridge_test_mount
#include REFUSE_IMPLEMENTATION_PATH
#undef puffs_cc_getcc
#undef puffs_cc_getcaller
#undef puffs_mount

/* Caller and failed-mount seams are substituted; callbacks are the built source. */
static pid_t request_pid;
static int caller_error;
struct puffs_cc *bridge_test_getcc(struct puffs_usermount *pu)
{ (void)pu; return (struct puffs_cc *)(uintptr_t)1; }
int bridge_test_getcaller(struct puffs_cc *cc,pid_t *pid,lwpid_t *lwp)
{ (void)cc; if(caller_error)return -1;if(pid)*pid=request_pid;if(lwp)*lwp=0;return 0; }
int bridge_test_mount(struct puffs_usermount *pu,const char *path,int flags,puffs_cookie_t cookie)
{ (void)pu;(void)path;(void)flags;(void)cookie;errno=EPERM;return -1; }
static int result, file_calls, dir_calls, barrier_calls, observed_data;
static int open_calls,release_calls,open_error,release_error;
static int dir_open_calls,dir_release_calls;
static off_t written_offset;
static int test_open(const char *path,struct fuse_file_info *fi)
{ assert(!strcmp(path,"/file"));open_calls++;assert(fi->fh==(open_calls==1?0:0x12345678));assert(!(fi->flags&(O_CREAT|O_EXCL|O_TRUNC)));if(open_error){fi->fh=0xbad;return open_error;}fi->fh=0x12345678;return 0; }
static int test_release(const char *path,struct fuse_file_info *fi)
{ (void)path;assert(fi->fh==0x12345678);release_calls++;return release_error; }
static int test_dir_open(const char *path,struct fuse_file_info *fi)
{ (void)path;dir_open_calls++;assert(fi->fh==(dir_open_calls==1?0:0x87654321));fi->fh=0x87654321;return 0; }
static int test_dir_release(const char *path,struct fuse_file_info *fi)
{ (void)path;assert(fi->fh==0x87654321);dir_release_calls++;return 0; }
static int test_write(const char *path,const char *buf,size_t size,off_t offset,struct fuse_file_info *fi)
{ (void)path;(void)buf;assert(fi->fh==0x12345678);written_offset=offset;return size; }
static uint64_t observed_handle;
static pid_t observed_pid;
static int test_file_sync(const char *path,int data,struct fuse_file_info *fi)
{ assert(path && !strcmp(path,"/file"));file_calls++;observed_handle=fi?fi->fh:0;observed_data=data;observed_pid=fuse_get_context()->pid;return result; }
static int test_dir_sync(const char *path,int data,struct fuse_file_info *fi)
{ if(!path){assert(!fi && !data);barrier_calls++;}else{assert(!strcmp(path,"/file"));dir_calls++;} observed_handle=fi?fi->fh:0;observed_data=data;observed_pid=fuse_get_context()->pid;return result; }
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);
    struct fuse_operations_v38 op={.fsync=test_file_sync,.fsyncdir=test_dir_sync,.open=test_open,.release=test_release,.opendir=test_dir_open,.releasedir=test_dir_release,.write=test_write};
    char *argv[]={"bridge-control","-ofsname=bridge-control,default_permissions",NULL};struct fuse_args args=FUSE_ARGS_INIT(2,argv);
    struct fuse *f=__fuse_new(&args,&op,38,NULL);assert(f);
    assert(f->pu->pu_ops.puffs_node_fsync==puffs_fuse_node_fsync);
    assert(!refuse_lkl_owner_only(f));
    f->allow_other=1;assert(refuse_lkl_owner_only(f)==EACCES);f->allow_other=0;
    f->allow_root=1;assert(refuse_lkl_owner_only(f)==EACCES);f->allow_root=0;
    f->default_permissions=0;assert(refuse_lkl_owner_only(f)==EACCES);f->default_permissions=1;
    f->mount_uid=32767;assert(refuse_lkl_owner_only(f)==EACCES);f->mount_uid=geteuid();
    struct refusenode rn={.opencount=1,.flags=RN_OPEN};rn.file_info.fh=0x12345678;
    struct puffs_node pn={0};pn.pn_data=&rn;pn.pn_va.va_type=VREG;pn.pn_po.po_path="/file";
    struct puffs_kcred kc={0};kc.pkcr_type=PUFFCRED_TYPE_UUC;kc.pkcr_uuc.cr_uid=geteuid();kc.pkcr_uuc.cr_gid=getegid();
    const struct puffs_cred *cr=(const void*)&kc;
    request_pid=101;result=0;set_fuse_context_pid(f->pu);
    assert(!puffs_fuse_node_fsync(f->pu,&pn,cr,0,0,100));
    assert(file_calls==1 && observed_handle==0x12345678 && observed_data==0 && observed_pid==101);
    request_pid=202;result=-ENOSPC;set_fuse_context_pid(f->pu);
    assert(puffs_fuse_node_fsync(f->pu,&pn,cr,PUFFS_FSYNC_DATAONLY,0,0)==ENOSPC);
    assert(file_calls==2 && observed_data==1 && observed_pid==202);
    pn.pn_va.va_type=VDIR;result=-EIO;
    assert(puffs_fuse_node_fsync(f->pu,&pn,cr,0,0,0)==EIO);assert(dir_calls==1);
    rn.flags=0;result=0;
    assert(!puffs_fuse_node_fsync(f->pu,&pn,cr,0,0,0));assert(!observed_handle);
    request_pid=0;result=-EIO;set_fuse_context_pid(f->pu);
    assert(puffs_fuse_fs_sync(f->pu,0,cr)==EIO);assert(barrier_calls==1 && observed_pid==0);
    caller_error=1;fuse_get_context()->pid=999;set_fuse_context_pid(f->pu);assert(!fuse_get_context()->pid);caller_error=0;
    result=17;assert(puffs_fuse_fs_sync(f->pu,0,cr)==EIO);
    kc.pkcr_uuc.cr_uid=32767;assert(puffs_fuse_fs_sync(f->pu,0,cr)==EACCES);
    kc.pkcr_uuc.cr_uid=geteuid();
    pn.pn_va.va_type=VREG;rn.opencount=0;rn.flags=0;rn.file_info.fh=0xdead;
    request_pid=303;set_fuse_context_pid(f->pu);
    assert(!puffs_fuse_node_open(f->pu,&pn,1,cr));assert(open_calls==1 && rn.opencount==1 && (rn.flags&RN_OPEN));
    assert(!puffs_fuse_node_open(f->pu,&pn,2|O_CREAT|O_EXCL|O_TRUNC,cr));assert(open_calls==2 && rn.opencount==2);
    open_error=-EACCES;
    assert(puffs_fuse_node_open(f->pu,&pn,1,cr)==EACCES && rn.opencount==2 && rn.file_info.fh==0x12345678);
    open_error=0;
    assert(!puffs_fuse_node_close(f->pu,&pn,0,cr));assert(rn.opencount==1 && !release_calls && (rn.flags&RN_OPEN));
    pn.pn_va.va_size=5;uint8_t data[5]={0};size_t remaining=sizeof(data);
    assert(!puffs_fuse_node_write(f->pu,&pn,data,0,&remaining,cr,PUFFS_IO_APPEND));assert(!remaining && written_offset==5 && pn.pn_va.va_size==10);
    remaining=sizeof(data);assert(!puffs_fuse_node_write(f->pu,&pn,data,1,&remaining,cr,0));assert(!remaining && written_offset==1 && pn.pn_va.va_size==10);
    release_error=-EIO;
    assert(puffs_fuse_node_close(f->pu,&pn,0,cr)==EIO && release_calls==1 && rn.opencount==1 && (rn.flags&RN_OPEN));
    release_error=0;assert(!puffs_fuse_node_close(f->pu,&pn,0,cr));assert(release_calls==2 && !rn.opencount && !(rn.flags&RN_OPEN) && !rn.file_info.fh);
    assert(puffs_fuse_node_close(f->pu,&pn,0,cr)==EBADF && !rn.opencount);
    pn.pn_va.va_type=VDIR;rn.file_info.fh=0xdead;
    assert(!puffs_fuse_node_open(f->pu,&pn,1,cr));assert(!puffs_fuse_node_open(f->pu,&pn,1,cr));
    assert(dir_open_calls==2 && rn.opencount==2);
    assert(!puffs_fuse_node_close(f->pu,&pn,0,cr) && !dir_release_calls);
    assert(!puffs_fuse_node_close(f->pu,&pn,0,cr) && dir_release_calls==1);
    assert(__fuse_mount(f,"/never-mounted")==-1 && errno==EPERM);
    assert(refuse_lkl_bridge_version()==2);
    puts("uid1300 isolated ReFUSE: actual node/fs sync/errors/PID/foreign admission; every-open authentication contract/last-close/failed-open atomicity/release-error retention; append/explicit offsets and mount-failure return passed; no mount");
    /* Never mounted. Closing the process here cannot detach a mount. */
    return 0;
}
