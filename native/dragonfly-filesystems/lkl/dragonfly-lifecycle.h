/* Native mount observation for the foreground, externally published FUSE
 * channel. No function in this adapter mounts, unmounts or force-unmounts.
 */
#ifndef LKLFUSE_DRAGONFLY_LIFECYCLE_H
#define LKLFUSE_DRAGONFLY_LIFECYCLE_H
extern struct fuse_chan *fuse_kern_chan_new(int fd);
static const char *df_native_target;
static const char *df_native_alias;
static fsid_t df_native_fsid;
static int df_native_captured;
static int df_native_uncertain;
static volatile sig_atomic_t df_signal_seen;

static void df_signal(int signal_number)
{
    /* Keep the native session alive until its captured mount is absent. */
    df_signal_seen = signal_number;
}
static int df_native_clear(int require_capture)
{
    struct statfs *entries;
    int count = getmntinfo(&entries, MNT_NOWAIT);
    if (count <= 0 || (require_capture && !df_native_captured)) return -1;
    for (int i = 0; i < count; i++) {
        if (!strcmp(entries[i].f_mntonname, df_native_target) ||
            !strcmp(entries[i].f_mntfromname, df_native_alias) ||
            (df_native_captured &&
             !memcmp(&entries[i].f_fsid, &df_native_fsid, sizeof(df_native_fsid))))
            return -1;
    }
    return 0;
}
static void df_native_observe(void)
{
    struct statfs *entries;
    int count = getmntinfo(&entries, MNT_NOWAIT), matches = 0;
    if (count <= 0) { df_native_uncertain = 1; return; }
    if (df_native_captured) return;
    for (int i = 0; i < count; i++) {
        if (strcmp(entries[i].f_mntonname, df_native_target) ||
            strcmp(entries[i].f_mntfromname, df_native_alias)) continue;
        if (strcmp(entries[i].f_fstypename, "fuse")) {
            df_native_uncertain = 1; return;
        }
        df_native_fsid = entries[i].f_fsid;
        matches++;
    }
    if (matches == 1) {
        df_native_captured = 1;
        fprintf(stderr, "lklfuse: captured native FSID %d:%d source=%s target=%s\n",
                df_native_fsid.val[0], df_native_fsid.val[1], df_native_alias, df_native_target);
    } else if (matches > 1) df_native_uncertain = 1;
}
static int df_admin_ancestry(const char *path)
{
    char parent[PATH_MAX];
    if (!path || path[0] != '/' || strlen(path) >= sizeof(parent)) return -1;
    strcpy(parent, path);
    for (;;) {
        char *slash = strrchr(parent, '/');
        if (!slash) return -1;
        if (slash == parent) { parent[1] = 0; } else *slash = 0;
        struct stat st;
        if (lstat(parent, &st) || !S_ISDIR(st.st_mode) || st.st_uid != 0 ||
            (st.st_mode & 0022)) return -1;
        if (slash == parent) return 0;
    }
}
static int df_native_inputs(const char *target, const char *alias, int supplied,
                             int ready)
{
    struct stat from, device, held, mountpoint, pipe;
    char link[PATH_MAX];
    if (geteuid() != 0 || !target || !alias || strlen(target) >= 80 ||
        strlen(alias) >= 80 || strpbrk(target, ",\\\r\n") ||
        strpbrk(alias, ",\\\r\n") || df_admin_ancestry(target) ||
        df_admin_ancestry(alias) || supplied < 3 || ready < 3 || supplied == ready ||
        lstat(target, &mountpoint) || !S_ISDIR(mountpoint.st_mode) ||
        mountpoint.st_uid != 0 || (mountpoint.st_mode & 0022) ||
        lstat(alias, &from) || !S_ISLNK(from.st_mode) || from.st_uid != 0 ||
        readlink(alias, link, sizeof(link)) != 9 || memcmp(link, "/dev/fuse", 9) ||
        stat("/dev/fuse", &device) || fstat(supplied, &held) ||
        !S_ISCHR(held.st_mode) || held.st_dev != device.st_dev ||
        held.st_ino != device.st_ino || held.st_rdev != device.st_rdev ||
        (fcntl(supplied, F_GETFL) & O_ACCMODE) != O_RDWR ||
        fstat(ready, &pipe) || !S_ISFIFO(pipe.st_mode) || pipe.st_uid != 0)
        return -1;
    df_native_target = target;
    df_native_alias = alias;
    return df_native_clear(0);
}
#endif
