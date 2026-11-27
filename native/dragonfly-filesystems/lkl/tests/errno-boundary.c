#include <assert.h>
#include "../dragonfly-errno.h"
int main(void)
{
    assert(df_lkl_result(0, 0) == 0);
    assert(df_lkl_result(1234567, 0) == 1234567);
    assert(df_lkl_result(-LKL_ENOTEMPTY, 0) == -ENOTEMPTY);
    assert(df_lkl_result(-LKL_ENOSYS, 0) == -ENOSYS);
    assert(df_lkl_result(-LKL_EOPNOTSUPP, 0) == -EOPNOTSUPP);
    assert(df_lkl_result(-LKL_ENODATA, 1) == -ENOATTR);
    assert(df_lkl_result(-LKL_EACCES, 0) == -EACCES);
    assert(df_lkl_result(-LKL_EIO, 0) == -EIO);
    assert(df_lkl_result(-9999, 0) == -EIO);
    printf("native errno boundary ENOTEMPTY=%d ENOSYS=%d EOPNOTSUPP=%d ENOATTR=%d; positive result preserved; unknown diagnosed\n", ENOTEMPTY, ENOSYS, EOPNOTSUPP, ENOATTR);
    return 0;
}
