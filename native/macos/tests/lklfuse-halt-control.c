// SPDX-License-Identifier: GPL-2.0-only
/* Production main/callbacks with one simulated pre-shutdown halt failure,
 * followed by the real native LKL halt. No emulated success or root execution. */
#define main driver_program_main
#define lkl_sys_halt control_halt
#define lkl_cleanup control_cleanup
#define close control_close
#include "lklfuse-darwin.c"
#undef main
#undef lkl_sys_halt
#undef lkl_cleanup
#undef close
#include <assert.h>
extern long lkl_sys_halt(void);
extern void lkl_cleanup(void);
extern int close(int);
static int halt_calls, cleanup_calls, backing_close_calls;
static long actual_halt_result = -1;
long control_halt(void) {
  halt_calls++;
  assert(!cleanup_calls && !backing_close_calls);
  assert(backing_fd >= 0 && fcntl(backing_fd, F_GETFD) >= 0);
  if (halt_calls == 1) {
    assert(lkl_is_running());
    fprintf(stderr, "CONTROL_HALT simulated=-%d backing_open=1 cleanup=0\n",
            LKL_EAGAIN);
    return -LKL_EAGAIN;
  }
  assert(halt_calls == 2 && sticky_shutdown_error);
  actual_halt_result = lkl_sys_halt();
  assert(!actual_halt_result && !lkl_is_running());
  return actual_halt_result;
}
void control_cleanup(void) {
  assert(halt_calls == 2 && !actual_halt_result && !backing_close_calls);
  cleanup_calls++;
  lkl_cleanup();
}
int control_close(int fd) {
  if (fd == backing_fd && backing_fd >= 0) {
    assert(halt_calls == 2 && !actual_halt_result && cleanup_calls == 1);
    backing_close_calls++;
  }
  return close(fd);
}
int main(int argc, char **argv) {
  int result = driver_program_main(argc, argv);
  assert(result == 1 && halt_calls == 2 && !actual_halt_result);
  assert(cleanup_calls == 1 && backing_close_calls == 1 && sticky_shutdown_error);
  printf("HALT_CONTROL_PASS negative_then_actual0=1 retained_before_retry=1 "
         "cleanup_after_zero=1 backing_close_after_cleanup=1 production_exit=1\n");
  return 0;
}
