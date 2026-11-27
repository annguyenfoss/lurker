/* Native generated-UAPI and optional RAM-only shutdown contract. No host disk. */
#include <assert.h>
#include <endian.h>
#include <netinet/in.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <unistd.h>
#include <lkl.h>
#include <lkl_host.h>

_Static_assert(sizeof(struct lkl_stat) == 128, "Linux generic stat size");
_Static_assert(offsetof(struct lkl_stat, st_ino) == 8, "Linux inode offset");
_Static_assert(offsetof(struct lkl_stat, st_mode) == 16, "Linux mode offset");
_Static_assert(offsetof(struct lkl_stat, st_nlink) == 20, "Linux link offset");
_Static_assert(offsetof(struct lkl_stat, st_uid) == 24, "Linux UID offset");
_Static_assert(offsetof(struct lkl_stat, st_gid) == 28, "Linux GID offset");
_Static_assert(offsetof(struct lkl_stat, st_size) == 48, "Linux file size offset");
_Static_assert(sizeof(struct lkl_icmphdr) == 8, "Linux ICMP layout");
_Static_assert(offsetof(struct lkl_icmphdr, un.frag.mtu) == 6, "Retain Linux padding field");
_Static_assert(sizeof(struct lkl_in_pktinfo) == 12, "Linux packet info layout");
_Static_assert(offsetof(struct lkl_in_pktinfo, ipi_spec_dst) == 4, "Linux source address offset");
_Static_assert(offsetof(struct lkl_in_pktinfo, ipi_addr) == 8, "Linux packet address offset");
_Static_assert(sizeof(struct lkl_siginfo) == 128, "Linux signal layout");
_Static_assert(sizeof(struct lkl_sigevent) == 64, "Linux signal event layout");
_Static_assert(offsetof(struct lkl_linux_dirent64, d_name) == 19, "Linux directory entry layout");

#ifndef LURKER_LKL_ABI_ONLY
static void (*underlying_quiesce)(void);
static unsigned quiesce_calls;
static void measured_quiesce(void)
{
	assert(!lkl_is_running());
	++quiesce_calls;
	underlying_quiesce();
}
#endif
int main(void)
{
	setvbuf(stdout, NULL, _IONBF, 0);
	assert(getuid() == 1300 && geteuid() == 1300);
	siginfo_t native = {0};
	native.si_pid = 123; native.si_uid = 456;
	assert(native.si_pid == 123 && native.si_uid == 456);
	struct sigaction action = {0}; action.sa_handler = SIG_IGN;
	assert(action.sa_handler == SIG_IGN);
	puts("native generated Linux stat/signal/packet/directory ABI and restored native aliases passed");
#ifndef LURKER_LKL_ABI_ONLY
	assert(lkl_host_ops.timer_quiesce);
	underlying_quiesce = lkl_host_ops.timer_quiesce;
	lkl_host_ops.timer_quiesce = measured_quiesce;
	assert(lkl_init(&lkl_host_ops) == 0);
	assert(lkl_start_kernel("mem=32M loglevel=8") == 0);
	assert(lkl_is_running() && lkl_sys_getpid() > 0);
	struct __lkl__kernel_timespec delay = {.tv_sec = 0, .tv_nsec = 5000000};
	unsigned long long before = lkl_host_ops.time();
	assert(lkl_sys_nanosleep(&delay, NULL) == 0);
	assert(lkl_host_ops.time() - before >= 4500000);
	/* A rejected reboot has not drained workers or surrendered the instance. */
	assert(lkl_sys_setresuid(-1, 1300, -1) == 0);
	assert(lkl_sys_halt() == -LKL_EPERM);
	assert(lkl_is_running() && quiesce_calls == 0);
	assert(lkl_sys_getpid() > 0);
	assert(lkl_sys_setresuid(-1, 0, -1) == 0);
	assert(lkl_sys_halt() == 0);
	assert(!lkl_is_running() && quiesce_calls == 1);
	lkl_cleanup();
	puts("RAM-only native LKL boot/nanosleep/rejected-halt/retry and post-CPU pre-free-memory callback passed");
#endif
	return 0;
}
