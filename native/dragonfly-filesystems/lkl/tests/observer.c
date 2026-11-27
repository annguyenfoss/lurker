/* Native unit contract, with only the kernel query results controlled. */
#include <sys/types.h>
#include <sys/mount.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#include <pthread.h>
#include <signal.h>
#include <fcntl.h>
#include <unistd.h>
#include <limits.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
static struct statfs rows[1];
static int rows_count=1, probe_errno=EOPNOTSUPP, query_error;
static int controlled_getmntinfo(struct statfs **out, int flags)
{ assert(flags==MNT_NOWAIT); *out=rows; return query_error ? 0 : rows_count; }
static int controlled_fhstatfs(const fhandle_t *handle,struct statfs *unused)
{ (void)unused;assert(handle->fh_fsid.val[0]==991 && handle->fh_fsid.val[1]==123);errno=probe_errno;return -1; }
static int controlled_sysctl(const char *name,void *value,size_t *bytes,const void *in,size_t count)
{ assert(!strcmp(name,"jail.jailed") && *bytes==sizeof(int) && !in && !count);*(int *)value=0;return 0; }
static uid_t controlled_root(void){return 0;}
#define getmntinfo controlled_getmntinfo
#define fhstatfs controlled_fhstatfs
#define sysctlbyname controlled_sysctl
#define getuid controlled_root
#define geteuid controlled_root
#include "../dragonfly-lifecycle.h"
#undef getuid
#undef geteuid
static void reset(void)
{
    df_native_captured=df_native_typed=df_native_uncertain=df_native_absent=0;
    df_native_stopping=df_native_observer_error=0;
    rows_count=1;probe_errno=EOPNOTSUPP;query_error=0;
    memset(rows,0,sizeof(rows));strcpy(rows[0].f_mntonname,"/owned-target");
    strcpy(rows[0].f_mntfromname,"/owned-alias");strcpy(rows[0].f_fstypename,"fuse");
    rows[0].f_fsid=(fsid_t){{991,123}};
}
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);
    df_native_target="/owned-target";df_native_alias="/owned-alias";
    df_native_context_fd=open("/",O_RDONLY|O_DIRECTORY|O_CLOEXEC);
    assert(df_native_context_fd>=0 && fstat(df_native_context_fd,&df_native_root)==0);
    reset();assert(df_native_clear(1)<0 && df_native_captured && df_native_typed);
    rows_count=0; /* Native enumeration may legitimately contain other mounts. */
    struct statfs unrelated={0};strcpy(unrelated.f_mntonname,"/");rows[0]=unrelated;rows_count=1;
    assert(df_native_clear(1)<0); /* omitted unmounting FSID is still present */
    probe_errno=ESTALE;assert(df_native_clear(1)==0);
    probe_errno=EPERM;assert(df_native_clear(1)<0 && df_native_uncertain);
    probe_errno=ESTALE;query_error=1;assert(df_native_clear(1)<0);
    query_error=0;strcpy(rows[0].f_mntonname,"/owned-target");
    assert(df_native_clear(1)<0); /* replacement or moved alias retains */
    reset();probe_errno=ESTALE;
    assert(df_native_clear(1)<0 && df_native_captured && !df_native_typed);
    strcpy(rows[0].f_mntonname,"/");strcpy(rows[0].f_mntfromname,"/base");
    assert(df_native_clear(1)<0); /* no positive FUSE handshake, no release */
    reset();strcpy(rows[0].f_fstypename,"hammer2");
    assert(df_native_clear(1)<0 && !df_native_captured && df_native_uncertain);
    reset();strcpy(rows[0].f_mntonname,"/");strcpy(rows[0].f_mntfromname,"/base");
    assert(df_native_begin()==0);usleep(30000);assert(df_native_clear(1)<0);
    df_native_end();assert(!df_native_running);
    __atomic_store_n(&df_native_observer_error,1,__ATOMIC_RELEASE);
    assert(df_native_clear(0)<0);
    assert(close(df_native_context_fd)==0);
    puts("native observer: omitted-but-present retained; only prior typed FUSE plus empty inventory and ESTALE clears; errors/replace/untyped retain; observer starts/joins outside callbacks");
    return 0;
}
