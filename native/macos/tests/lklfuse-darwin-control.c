// SPDX-License-Identifier: GPL-2.0-only
/* Native API boundary controls with synthetic request envelopes, real LKL VFS,
 * native kernel group snapshots and the unchanged production callback bodies.
 * This is not an OS-mounted request or an installed driver. */
#define main driver_program_main
#define fuse_get_context control_get_context
#define fuse_getgroups control_getgroups
#include "lklfuse-darwin.c"
#undef main
#undef fuse_get_context
#undef fuse_getgroups
#include <assert.h>
#include <sys/wait.h>
static struct fuse_context control_context;
struct fuse_context *control_get_context(void) { return &control_context; }
int control_getgroups(int size, gid_t list[]) {
  (void)size;
  (void)list;
  return -ENOSYS;
}
static void caller(void) {
  memset(&control_context, 0, sizeof control_context);
  control_context.uid = getuid();
  control_context.gid = getgid();
  control_context.pid = getpid();
  control_context.umask = 077;
}
static void restored(const struct guard *base) {
  struct guard now = {0};
  assert(!save_credentials(&now));
  assert(now.ruid == base->ruid && now.euid == base->euid &&
         now.suid == base->suid && now.fsuid == base->fsuid);
  assert(now.rgid == base->rgid && now.egid == base->egid &&
         now.sgid == base->sgid && now.fsgid == base->fsgid);
  assert(now.ngroups == base->ngroups &&
         !memcmp(now.groups, base->groups, now.ngroups * sizeof(*now.groups)));
  assert(now.mask == base->mask && !credential_poison);
}
static void native_snapshot_edges(void) {
  struct kinfo_proc a, b;
  assert(!kernel_snapshot(getpid(), &a));
  b = a;
  assert(same_kernel_credentials(&a, &b));
  b.kp_proc.p_starttime.tv_usec = (b.kp_proc.p_starttime.tv_usec + 1) % 1000000;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_proc.p_stat = SZOMB;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_proc.p_flag |= P_WEXIT;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_eproc.e_ucred.cr_uid++;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_eproc.e_ucred.cr_groups[0]++;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_eproc.e_pcred.p_ruid++;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_eproc.e_pcred.p_svuid++;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_eproc.e_pcred.p_rgid++;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_eproc.e_pcred.p_svgid++;
  assert(!same_kernel_credentials(&a, &b));
  b = a;
  b.kp_eproc.e_ucred.cr_ngroups = NGROUPS + 1;
  assert(!same_kernel_credentials(&b, &b));
  if (a.kp_eproc.e_ucred.cr_ngroups > 1) {
    b = a;
    b.kp_eproc.e_ucred.cr_groups[1]++;
    assert(!same_kernel_credentials(&a, &b));
  }
  gid_t groups[GROUP_LIMIT];
  assert(kernel_request_groups(getpid(), getuid(), getgid(), groups, 0) ==
         -E2BIG);
  pid_t child = fork();
  assert(child >= 0);
  if (!child)
    _exit(0);
  int zombie = 0;
  for (int i = 0; i < 1000; i++) {
    size_t size = sizeof b;
    int mib[] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, child};
    memset(&b, 0, sizeof b);
    assert(!sysctl(mib, 4, &b, &size, NULL, 0));
    if (size == sizeof b && b.kp_proc.p_stat == SZOMB) {
      zombie = 1;
      break;
    }
    usleep(1000);
  }
  assert(zombie && kill(child, 0) == 0);
  assert(kernel_request_groups(child, getuid(), getgid(), groups,
                               GROUP_LIMIT) == -ESRCH);
  int status;
  assert(waitpid(child, &status, 0) == child && WIFEXITED(status));
  assert(kernel_request_groups(child, getuid(), getgid(), groups,
                               GROUP_LIMIT) == -ESRCH);
  printf("NATIVE_SNAPSHOT_EDGES birth_change=refused zombie_live_pid=refused "
         "disappeared=refused exiting=refused credential_tuple_change=refused "
         "groups_overflow=refused capacity=refused\n");
}
static void native_mount_identity_edges(void) {
  char path[] = "/private/tmp/lurker-native-mount-gate-XXXXXX";
  assert(mkdtemp(path));
  struct native_mount_watch w = {.point = path, .captured_ok = 1};
  assert(!lstat(path, &w.underlay));
  assert(!native_mount_gone(&w));
  w.captured_ok = 0;
  assert(native_mount_gone(&w) == -ESTALE);
  w.captured_ok = 1;
  w.error = -EIO;
  assert(native_mount_gone(&w) == -EIO);
  w.error = 0;
  struct statfs *v = NULL;
  int n = native_mounts(&v);
  assert(n > 0);
  w.captured = v[0];
  free(v);
  assert(native_mount_gone(&w) == -EBUSY);
  memset(&w.captured, 0, sizeof w.captured);
  char moved[PATH_MAX];
  size_t len = strlen(path);
  memcpy(moved, path, len);
  memcpy(moved + len, "-moved", 7);
  assert(!rename(path, moved));
  assert(!mkdir(path, 0700));
  assert(native_mount_gone(&w) == -ESTALE);
  assert(!rmdir(path));
  assert(!rename(moved, path));
  assert(!native_mount_gone(&w));
  assert(!rmdir(path));
  puts("NATIVE_MOUNT_GATE absent_identity=refused "
       "retained_fsid_anywhere=refused replaced_underlay=refused "
       "original_underlay=accepted");
}
int main(int argc, char **argv) {
  assert(argc == 3 && getuid() && geteuid() == getuid());
  native_snapshot_edges();
  native_mount_identity_edges();
  struct stat source;
  assert(!lstat(argv[1], &source) && S_ISREG(source.st_mode) &&
         source.st_uid == getuid());
  backing_fd = open(argv[1], O_RDWR | O_NOFOLLOW | O_CLOEXEC);
  assert(backing_fd >= 0 && !flock(backing_fd, LOCK_EX | LOCK_NB));
  mount_identity.uid = getuid();
  mount_identity.gid = getgid();
  mount_identity.pid = getpid();
  mount_identity.mask = 022;
  mount_identity.identity_source = 3;
  gid_t groups[GROUP_LIMIT];
  int n =
      kernel_request_groups(getpid(), getuid(), getgid(), groups, GROUP_LIMIT);
  assert(n >= 0);
  mount_identity.ngroups = n;
  for (int i = 0; i < n; i++)
    mount_identity.groups[i] = groups[i];
  assert(!setup_mount(argv[2], 0));
  struct guard base = {0};
  assert(!save_credentials(&base));
  caller();
  struct fuse_file_info fi = {.flags = O_RDWR};
  assert(!native_create("/lfresh-user/edge-file", 0666, &fi));
  restored(&base);
  struct stat st = {0};
  assert(!native_getattr("/lfresh-user/edge-file", &st, NULL));
  assert(st.st_uid == getuid() && st.st_gid == getgid() &&
         (st.st_mode & 0777) == 0600);
  assert(native_write(NULL, "data", 4, 0, &fi) == 4);
  restored(&base);
  control_context.pid = 0;
  char b[8];
  assert(native_read(NULL, b, sizeof b, 0, &fi) == -ESRCH);
  assert(native_unlink("/lfresh-user/edge-file") == -ESRCH);
  assert(!native_flush(NULL, &fi));
  restored(&base);
  assert(!native_release(NULL, &fi));
  assert(native_read(NULL, b, sizeof b, 0, &fi) == -EBADF);
  restored(&base);
  caller();
  control_context.pid = INT_MAX;
  assert(native_mkdir("/lfresh-user/dead-mutation", 0700) == -ESRCH);
  restored(&base);
  caller();
  control_context.uid = 0;
  assert(native_unlink("/lfresh-user/edge-file") == -EACCES);
  restored(&base);
  caller();
  assert(!native_mkdir("/lfresh-user/nonempty", 0700));
  struct fuse_file_info child = {.flags = O_RDWR};
  assert(!native_create("/lfresh-user/nonempty/child", 0600, &child));
  assert(!native_release(NULL, &child));
  assert(native_rmdir("/lfresh-user/nonempty") == -ENOTEMPTY);
  assert(native_getxattr("/lfresh-user/edge-file", "com.lurker.missing", NULL,
                         0) == -ENOATTR);
  restored(&base);
  struct fuse_file_info again = {.flags = O_RDWR}, dest = {.flags = O_RDWR};
  assert(!native_open("/lfresh-user/edge-file", &again));
  assert(!native_create("/lfresh-user/dest", 0600, &dest));
  assert(native_fallocate(NULL, 0, 0, 4096, &again) == -ENOTSUP);
  assert(native_copy_file_range(NULL, &again, 0, NULL, &dest, 0, 4, 0) ==
         -ENOSYS);
  assert(!native_getattr("/lfresh-user/dest", &st, &dest) && st.st_size == 0);
  restored(&base);
  assert(!native_release(NULL, &again));
  assert(!native_release(NULL, &dest));
  restored(&base);
  control_context.pid = 0;
  assert(!native_getattr("/", &st, NULL));
  assert(native_getattr("/lfresh-user/edge-file", &st, NULL) == -ESRCH);
  assert(!handles);
  restored(&base);
  caller();
  struct fuse_file_info broken = {.flags = O_RDWR};
  assert(!native_open("/lfresh-user/edge-file", &broken));
  struct held_handle *bh = find_handle(broken.fh, 0);
  assert(bh && !lkl_sys_close(bh->raw));
  assert(native_fsync(NULL, 0, &broken) == -EBADF && sticky_io_error);
  assert(native_release(NULL, &broken) == -EBADF);
  remove_handle(bh);
  restored(&base);
  struct fuse_file_info broken_dir = {0};
  assert(!native_opendir("/lfresh-user/nonempty", &broken_dir));
  bh = find_handle(broken_dir.fh, 1);
  assert(bh && !lkl_sys_close(bh->raw));
  assert(native_fsyncdir(NULL, 0, &broken_dir) == -EBADF && sticky_io_error);
  assert(native_releasedir(NULL, &broken_dir) == -EBADF);
  remove_handle(bh);
  restored(&base);
  assert(!handles);
  puts("SYNC_ERROR_EDGES file_fsync=EBADF directory_fsync=EBADF "
       "sticky_failure=1 production_exit_would_fail=1");
  int drain = normal_drain();
  assert(!drain);
  if (original_root >= 0)
    lkl_sys_close(original_root);
  lkl_sys_halt();
  lkl_cleanup();
  assert(!close(backing_fd));
  printf("API_EDGE_PASS filesystem=%s uid=%u creation=0600 ENOTEMPTY=%d "
         "ENOATTR=%d ENOTSUP=%d ENOSYS=%d pid0_data_refused=1 held_cleanup=1 "
         "dead_path_refused=1 forged_root_refused=1 stale_handle_refused=1 "
         "restored_groups_umask=1 normal_drain=0\n",
         argv[2], getuid(), ENOTEMPTY, ENOATTR, ENOTSUP, ENOSYS);
  return 0;
}
