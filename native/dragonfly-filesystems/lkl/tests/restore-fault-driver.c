#include "restore-fault.h"
#define main df_production_main
#include "../lklfuse.c"
#undef main
#undef lkl_sys_setresuid
static void toggle_restore_fault(int signal_number)
{
    (void)signal_number;
    df_test_restore_fault = !df_test_restore_fault;
}
int main(int argc, char **argv)
{
    struct sigaction action = {0};
    action.sa_handler = toggle_restore_fault;
    sigemptyset(&action.sa_mask);
    assert(sigaction(SIGUSR2, &action, NULL) == 0);
    return df_production_main(argc, argv);
}
