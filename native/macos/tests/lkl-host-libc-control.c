// SPDX-License-Identifier: GPL-2.0-only
/* Link against the pinned Darwin archive; run as the login user. This checks
 * direct host references, including variadic formatting before kernel init.
 * It never opens a backing file, mounts a filesystem or changes host state. */
#define _DARWIN_C_SOURCE
#include <lkl.h>
#include <lkl_host.h>
#include <assert.h>
#include <dlfcn.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
struct symbol {const char *name; const void *address;};
static int format(char *out,size_t n,const char *fmt,...){va_list ap;va_start(ap,fmt);int r=vsnprintf(out,n,fmt,ap);va_end(ap);return r;}
static void check_host_libc(void){
 const struct symbol symbols[]={
  {"snprintf",snprintf},{"vsnprintf",vsnprintf},{"sprintf",sprintf},{"vsprintf",vsprintf},
  {"strcat",strcat},{"strchr",strchr},{"strcmp",strcmp},{"strcpy",strcpy},
  {"strlen",strlen},{"strncat",strncat},{"strncmp",strncmp},{"strncpy",strncpy},
  {"strnlen",strnlen},{"strrchr",strrchr},{"bsearch",bsearch},
  {"memcpy",memcpy},{"memset",memset},{"memmove",memmove},
 };
 for(size_t i=0;i<sizeof symbols/sizeof *symbols;i++){
  Dl_info info={0};assert(dladdr(symbols[i].address,&info));assert(info.dli_fname);
  assert(!strncmp(info.dli_fname,"/usr/lib/",9)||!strncmp(info.dli_fname,"/System/Library/",16));
  printf("HOST_LIBC symbol=%s image=%s\n",symbols[i].name,info.dli_fname);
 }
 char text[64];assert(snprintf(text,sizeof text,"native:%s:%d","darwin",42)==16);assert(!strcmp(text,"native:darwin:42"));
 assert(format(text,sizeof text,"%s:%u:%zu","before-init",7U,(size_t)19)==16);assert(!strcmp(text,"before-init:7:19"));
}
int main(void){
 assert(getuid()&&geteuid()==getuid());check_host_libc();
 assert(!lkl_init(&lkl_host_ops));assert(!lkl_start_kernel("mem=128M"));
 check_host_libc();lkl_sys_halt();lkl_cleanup();check_host_libc();puts("HOST_LIBC_PASS preinit=1 running=1 postcleanup=1");return 0;
}
