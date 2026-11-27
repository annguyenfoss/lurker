#define main df_production_main
#include "../lklfuse.c"
#undef main
#include <assert.h>
static void run_case(int unknown)
{
    char options[256];snprintf(options,sizeof(options),"type=xfs,source_alias=/unopened-alias,ready_fd=7,allow_other,default_permissions,direct_io%s",unknown?",unrecognized_native_test_option":"");
    char *argv[]={"native-constructor-options","/unopened-image","/tmp","-s","-f","-o",options,NULL};
    struct fuse_args args=FUSE_ARGS_INIT(7,argv);char *target=NULL;int foreground,multithreaded;
    lklfuse=(struct lklfuse){.mb=128,.ready_fd=-1};
    assert(fuse_opt_parse(&args,&lklfuse,lklfuse_opts,lklfuse_opt_proc)==0);
    assert(fuse_parse_cmdline(&args,&target,&multithreaded,&foreground)==0 && foreground && !multithreaded);
    int generated=0;
    for(int i=0;i<args.argc;i++)generated|=strstr(args.argv[i],"fsname=")!=NULL;
    assert(generated && remove_kernel_name(&args)==0);
    for(int i=0;i<args.argc;i++)assert(!strstr(args.argv[i],"fsname="));
    assert(lklfuse.native_allow_other && lklfuse.native_default_permissions && !strcmp(lklfuse.source_alias,"/unopened-alias"));
    int fd=open("/dev/null",O_RDWR|O_CLOEXEC);assert(fd>=0);
    struct fuse_chan *channel=fuse_kern_chan_new(fd);assert(channel);
    struct fuse_operations operations={0};struct fuse *fuse=fuse_new(channel,&args,&operations,sizeof(operations),NULL);
    if(unknown){assert(!fuse);fuse_chan_destroy(channel);}else{assert(fuse);fuse_destroy(fuse);}
    errno=0;assert(fcntl(fd,F_GETFD)==-1 && errno==EBADF);
    fuse_opt_free_args(&args);free(target);free((void *)lklfuse.file);free((void *)lklfuse.type);free((void *)lklfuse.source_alias);
}
int main(void)
{
    assert(getuid()==1300 && geteuid()==1300);run_case(0);run_case(1);
    puts("native parse→generated-fsname removal→unmounted constructor passes; unrelated unknown option still rejected; no source opened/mount published");return 0;
}
