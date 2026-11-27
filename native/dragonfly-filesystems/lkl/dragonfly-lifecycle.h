/* Native mount observation for the foreground, externally published FUSE
 * channel. No function in this adapter mounts, unmounts or force-unmounts.
 */
#ifndef LKLFUSE_DRAGONFLY_LIFECYCLE_H
#define LKLFUSE_DRAGONFLY_LIFECYCLE_H
extern struct fuse_chan *fuse_kern_chan_new(int fd);
static const char *df_native_target;
static const char *df_native_alias;
static fsid_t df_native_fsid;
static int df_native_captured, df_native_typed, df_native_uncertain;
static int df_native_absent, df_native_running, df_native_stopping;
static int df_native_observer_error;
static int df_native_context_fd = -1;
static struct stat df_native_root;
static pthread_t df_native_thread;
static pthread_mutex_t df_native_lock = PTHREAD_MUTEX_INITIALIZER;
static volatile sig_atomic_t df_signal_seen;

static void df_signal(int signal_number)
{
    /* Keep the native session alive until its captured mount is absent. */
    df_signal_seen = signal_number;
}
/* This is deliberately FUSE-specific. On the qualified stock kernel,
 * vfs_getvfs looks in the global FSID tree, including unmounting entries;
 * FUSE has only the default fhtovp stub, which returns EOPNOTSUPP without
 * contacting the server. ESTALE is useful only after that exact captured
 * FUSE instance has returned EOPNOTSUPP in a qualified admin context.
 * A synthetic handle on an ordinary filesystem can return ESTALE even
 * while mounted, so this must never become a generic absence query. */
static int df_native_fsid_present(const fsid_t *fsid)
{
    fhandle_t handle = {0};
    struct statfs unused;
    handle.fh_fsid = *fsid;
    errno = 0;
    int result = fhstatfs(&handle, &unused), error = errno;
    if (result == -1 && error == EOPNOTSUPP) return 1;
    if (result == -1 && error == ESTALE) return 0;
    return -1;
}
static int df_native_context(void)
{
    int jailed = -1;
    size_t bytes = sizeof(jailed);
    struct stat root, held;
    return getuid() == 0 && geteuid() == 0 &&
        sysctlbyname("jail.jailed", &jailed, &bytes, NULL, 0) == 0 &&
        bytes == sizeof(jailed) && jailed == 0 &&
        df_native_context_fd >= 0 && fstat(df_native_context_fd, &held) == 0 &&
        stat("/", &root) == 0 && root.st_dev == df_native_root.st_dev &&
        root.st_ino == df_native_root.st_ino && held.st_dev == root.st_dev &&
        held.st_ino == root.st_ino;
}
/* Only the native observer calls this after it starts. In particular no
 * served callback calls getmntinfo, fhstatfs, or waits on this mutex. */
static void df_native_scan(void)
{
    struct statfs *entries;
    int qualified = df_native_context();
    int count = qualified ? getmntinfo(&entries, MNT_NOWAIT) : -1;
    if (pthread_mutex_lock(&df_native_lock)) {
        __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE); return;
    }
    df_native_absent = 0;
    if (count <= 0) {
        df_native_uncertain = 1;
        if (pthread_mutex_unlock(&df_native_lock))
            __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE);
        return;
    }
    int collisions = 0, candidates = 0;
    fsid_t candidate = {0};
    for (int i = 0; i < count; i++) {
        int target = !strcmp(entries[i].f_mntonname, df_native_target);
        int alias = !strcmp(entries[i].f_mntfromname, df_native_alias);
        int owned = df_native_captured && !memcmp(&entries[i].f_fsid,
            &df_native_fsid, sizeof(df_native_fsid));
        if (target || alias || owned) collisions++;
        if (target && alias) {
            if (strcmp(entries[i].f_fstypename, "fuse"))
                df_native_uncertain = 1;
            else { candidate = entries[i].f_fsid; candidates++; }
        }
    }
    if (!df_native_captured && candidates == 1) {
        df_native_fsid = candidate;
        df_native_captured = 1;
    } else if (candidates > 1) df_native_uncertain = 1;
    if (df_native_captured) {
        int present = df_native_fsid_present(&df_native_fsid);
        if (present == 1 && !df_native_typed) {
            df_native_typed = 1;
            fprintf(stderr, "lklfuse: captured native FSID %d:%d source=%s target=%s fhstatfs=EOPNOTSUPP\n",
                df_native_fsid.val[0], df_native_fsid.val[1], df_native_alias, df_native_target);
        } else if (present < 0 || (!df_native_typed && present == 0)) {
            df_native_uncertain = 1;
        }
        /* NOWAIT skips unmounting entries. It is never enough alone. */
        df_native_absent = !collisions && df_native_typed && present == 0;
    } else df_native_absent = !collisions;
    if (pthread_mutex_unlock(&df_native_lock))
        __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE);
}
static int df_native_clear(int require_capture)
{
    if (__atomic_load_n(&df_native_observer_error, __ATOMIC_ACQUIRE)) return -1;
    if (!df_native_running) df_native_scan();
    if (pthread_mutex_lock(&df_native_lock)) {
        __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE); return -1;
    }
    int result = df_native_absent && (!require_capture ||
        (df_native_captured && df_native_typed)) ? 0 : -1;
    if (pthread_mutex_unlock(&df_native_lock)) {
        __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE); return -1;
    }
    return __atomic_load_n(&df_native_observer_error, __ATOMIC_ACQUIRE) ? -1 : result;
}
static void *df_native_observer(void *unused)
{
    (void)unused;
    for (;;) {
        if (pthread_mutex_lock(&df_native_lock)) {
            __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE); return NULL;
        }
        int stopping = df_native_stopping;
        if (pthread_mutex_unlock(&df_native_lock)) {
            __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE); return NULL;
        }
        if (stopping) return NULL;
        df_native_scan();
        usleep(10000);
    }
}
static int df_native_begin(void)
{
    if (df_native_clear(0)) return -1;
    int error = pthread_create(&df_native_thread, NULL, df_native_observer, NULL);
    if (error) { errno = error; perror("lklfuse: native observer"); return -1; }
    df_native_running = 1;
    return 0;
}
static void df_native_end(void)
{
    if (!df_native_running) return;
    for (;;) {
        int error = pthread_mutex_lock(&df_native_lock);
        if (!error) {
            df_native_stopping = 1;
            error = pthread_mutex_unlock(&df_native_lock);
            if (!error) error = pthread_join(df_native_thread, NULL);
        }
        if (!error) { df_native_running = 0; return; }
        /* Joining is a cleanup gate too: retain resources on uncertainty. */
        __atomic_store_n(&df_native_observer_error, 1, __ATOMIC_RELEASE);
        fprintf(stderr, "lklfuse: retaining channel/source after observer join error %d\n", error);
        usleep(50000);
    }
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
    df_native_context_fd = open("/", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (df_native_context_fd < 0 || fstat(df_native_context_fd, &df_native_root) ||
        !df_native_context()) return -1;
    df_native_target = target;
    df_native_alias = alias;
    return df_native_clear(0);
}
#endif
