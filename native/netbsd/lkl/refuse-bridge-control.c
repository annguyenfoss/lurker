#define _NETBSD_SOURCE
#include <sys/types.h>
#include <sys/vnode.h>
#include <fs/puffs/puffs_msgif.h>
#include <puffs.h>
#include <puffs_priv.h>
struct puffs_cc *bridge_test_getcc(struct puffs_usermount *);
int bridge_test_getcaller(struct puffs_cc *,pid_t *,lwpid_t *);
#define puffs_cc_getcc bridge_test_getcc
#define puffs_cc_getcaller bridge_test_getcaller
#include REFUSE_IMPLEMENTATION_PATH
#undef puffs_cc_getcc
#undef puffs_cc_getcaller

/* Only the kernel caller seam is substituted; forwarding is the built source. */
static pid_t request_pid;
static int caller_error;
struct puffs_cc *bridge_test_getcc(struct puffs_usermount *pu)
{ (void)pu; return (struct puffs_cc *)(uintptr_t)1; }
int bridge_test_getcaller(struct puffs_cc *cc,pid_t *pid,lwpid_t *lwp)
{ (void)cc; if(caller_error)return -1;if(pid)*pid=request_pid;if(lwp)*lwp=0;return 0; }
static int result, file_calls, dir_calls, barrier_calls, observed_data;
static uint64_t observed_handle;
static pid_t observed_pid;
static int test_file_sync(const char *path,int data,struct fuse_file_info *fi)
{ assert(path && !strcmp(path,"/file"));file_calls++;observed_handle=fi?fi->fh:0;observed_data=data;observed_pid=fuse_get_context()->pid;return result; }
static int test_dir_sync(const char *path,int data,struct fuse_file_info *fi)
{ if(!path){assert(!fi && !data);barrier_calls++;}else{assert(!strcmp(path,"/file"));dir_calls++;} observed_handle=fi?fi->fh:0;observed_data=data;observed_pid=fuse_get_context()->pid;return result; }
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);
    struct fuse_operations_v38 op={.fsync=test_file_sync,.fsyncdir=test_dir_sync};
    char *argv[]={"bridge-control","-ofsname=bridge-control",NULL};struct fuse_args args=FUSE_ARGS_INIT(2,argv);
    struct fuse *f=__fuse_new(&args,&op,38,NULL);assert(f);
    assert(f->pu->pu_ops.puffs_node_fsync==puffs_fuse_node_fsync);
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
    assert(refuse_lkl_bridge_version()==1);
    puts("uid1300 isolated ReFUSE actual node/fs sync forwarding: file/dir/dataonly/NULL-held/barrier/native errno/current PID/PID0/foreign admission passed; no mount");
    /* Never mounted. Closing the process here cannot detach a mount. */
    return 0;
}
