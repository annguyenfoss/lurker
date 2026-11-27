/* Read-only diagnostic for the exact stock kernel contract. This program
 * never treats a synthetic ordinary-filesystem handle as absence proof. */
#include <sys/types.h>
#include <sys/mount.h>
#include <sys/stat.h>
#include <sys/sysctl.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <assert.h>
int main(int argc,char **argv)
{
    int jailed=-1;size_t bytes=sizeof(jailed);struct stat root;
    assert(getuid()==0 && geteuid()==0 && sysctlbyname("jail.jailed",&jailed,&bytes,NULL,0)==0 && bytes==sizeof(jailed) && jailed==0 && stat("/",&root)==0);
    printf("qualified controller context uid=%u euid=%u jail=%d root_dev=%llu root_ino=%llu; nonchroot launch is explicit controller precondition\n",getuid(),geteuid(),jailed,(unsigned long long)root.st_dev,(unsigned long long)root.st_ino);
    struct statfs *entries;int count=getmntinfo(&entries,MNT_NOWAIT);assert(count>0);
    fhandle_t handle={0};int selected=0;
    if(argc==3) {
        char *end0,*end1;long a=strtol(argv[1],&end0,10),b=strtol(argv[2],&end1,10);
        assert(!*end0 && !*end1 && (int)a==a && (int)b==b);handle.fh_fsid=(fsid_t){{(int)a,(int)b}};
        for(int i=0;i<count;i++)selected+=!memcmp(&handle.fh_fsid,&entries[i].f_fsid,sizeof(fsid_t));
        printf("explicit retained control FSID %d:%d inventory_matches=%d\n",handle.fh_fsid.val[0],handle.fh_fsid.val[1],selected);
    } else {
        assert(argc==1);
        for(int i=0;i<count;i++)if(!strcmp(entries[i].f_mntonname,"/")){
            handle.fh_fsid=entries[i].f_fsid;selected++;
            printf("present base type=%s source=%s target=%s fsid=%d:%d\n",entries[i].f_fstypename,entries[i].f_mntfromname,entries[i].f_mntonname,handle.fh_fsid.val[0],handle.fh_fsid.val[1]);
        }
        assert(selected==1);
    }
    struct statfs unused;errno=0;int result=fhstatfs(&handle,&unused),error=errno;
    printf("fhstatfs synthetic-fid result=%d errno=%d (%s); no generic absence claim\n",result,error,strerror(error));
    return 0;
}
