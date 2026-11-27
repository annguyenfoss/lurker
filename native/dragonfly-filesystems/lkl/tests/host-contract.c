#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <signal.h>
#include <unistd.h>
#include <lkl.h>
#include <lkl_host.h>
#include <lkl/linux/icmp.h>
extern int lkl_is_running(void);
_Static_assert(sizeof(struct lkl_sigevent) == 64, "Linux sigevent ABI changed");
_Static_assert(sizeof(struct lkl_icmphdr) == 8, "Linux ICMP header ABI changed");
_Static_assert(offsetof(struct lkl_icmphdr, un.frag.mtu) == 6, "Native __unused altered Linux wire offset");
static void (*underlying_quiesce)(void);
static int quiesce_calls;
static void measured_quiesce(void) {
 assert(!lkl_is_running()); ++quiesce_calls; underlying_quiesce();
}
int main(void) {
 assert(getuid()==1300 && geteuid()==1300);
 assert(lkl_host_ops.timer_quiesce); underlying_quiesce=lkl_host_ops.timer_quiesce;
 lkl_host_ops.timer_quiesce=measured_quiesce;
 assert(lkl_start_kernel(&lkl_host_ops,"mem=16M loglevel=3")==0);
 assert(lkl_is_running()); assert(lkl_sys_getpid()>0);
 struct lkl_timespec delay={.tv_sec=0,.tv_nsec=5000000};
 assert(lkl_sys_nanosleep(&delay,NULL)==0);
 assert(lkl_sys_halt()==0); assert(!lkl_is_running()); assert(quiesce_calls==1);
 puts("native UAPI layouts and explicit pre-free-memory shutdown callback passed"); return 0;
}
