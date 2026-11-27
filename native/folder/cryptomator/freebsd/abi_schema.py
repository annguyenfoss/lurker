"""Native fields consumed by the upstream jFUSE adapters; never Linux layouts.

The C compiler supplies offsets, widths and alignment. `api` controls the Java
interface carrier only, independently of the actual native integer width.
"""

STRUCTS = {
    'stat': [('st_ino', 'long'), ('st_mode', 'int'), ('st_nlink', 'long'),
             ('st_uid', 'int'), ('st_gid', 'int'), ('st_size', 'long'),
             ('st_atim', 'timespec'), ('st_mtim', 'timespec'),
             ('st_ctim', 'timespec'), ('st_birthtim', 'timespec')],
    'timespec': [('tv_sec', 'long'), ('tv_nsec', 'long')],
    'statvfs': [(n, 'long') for n in ['f_bsize', 'f_frsize', 'f_blocks',
                                    'f_bfree', 'f_bavail', 'f_namemax']],
    'fuse_file_info': [('flags', 'int'), ('fh', 'long'), ('lock_owner', 'long')],
    'fuse_conn_info': [(n, 'int') for n in ['proto_major', 'proto_minor',
        'max_write', 'max_read', 'max_readahead', 'capable', 'want',
        'max_background', 'congestion_threshold', 'time_gran']],
    'fuse_config': [(n, 'int') for n in ['set_gid', 'gid', 'set_uid', 'uid',
        'set_mode', 'umask', 'intr', 'intr_signal', 'remember', 'hard_remove',
        'use_ino', 'readdir_ino', 'direct_io', 'kernel_cache', 'auto_cache',
        'ac_attr_timeout_set', 'nullpath_ok', 'no_rofd_flush']] +
        [(n, 'double') for n in ['entry_timeout', 'negative_timeout',
                                'attr_timeout', 'ac_attr_timeout']],
    'fuse_args': [('argc', 'int'), ('argv', 'address'), ('allocated', 'int')],
    'fuse_cmdline_opts': [(n, 'int') for n in ['singlethread', 'foreground',
        'debug', 'nodefault_subtype', 'show_version', 'show_help', 'clone_fd',
        'max_idle_threads', 'max_threads']] + [('mountpoint', 'address')],
}

# C signatures are independently type-checked against the installed fuse.h.
# P=pointer, I=int, U=unsigned int, M=mode_t, Z=size_t, O=off_t, V=void.
CALLBACKS = {
    'getattr': ('I', 'PPP'), 'readlink': ('I', 'PPZ'),
    'mkdir': ('I', 'PM'), 'unlink': ('I', 'P'), 'rmdir': ('I', 'P'),
    'symlink': ('I', 'PP'), 'rename': ('I', 'PPU'),
    'chmod': ('I', 'PMP'), 'chown': ('I', 'PUUP'),
    'truncate': ('I', 'POP'), 'open': ('I', 'PP'),
    'read': ('I', 'PPZOP'), 'write': ('I', 'PPZOP'),
    'statfs': ('I', 'PP'), 'flush': ('I', 'PP'),
    'release': ('I', 'PP'), 'fsync': ('I', 'PIP'),
    'setxattr': ('I', 'PPPZI'), 'getxattr': ('I', 'PPPZ'),
    'listxattr': ('I', 'PPZ'), 'removexattr': ('I', 'PP'),
    'opendir': ('I', 'PP'), 'readdir': ('I', 'PPPOPI'),
    'releasedir': ('I', 'PP'), 'fsyncdir': ('I', 'PIP'),
    'init': ('P', 'PP'), 'destroy': ('V', 'P'),
    'access': ('I', 'PI'), 'create': ('I', 'PMP'),
    'utimens': ('I', 'PPP'),
}

CONSTANTS = {
    'fcntl_h': ['O_RDONLY', 'O_WRONLY', 'O_RDWR', 'O_APPEND', 'O_CREAT',
                'O_TRUNC', 'O_EXCL', 'O_SYNC', 'O_DSYNC'],
    'errno_h': ['E2BIG', 'EACCES', 'EBADF', 'EEXIST', 'EINVAL', 'EIO',
                'EISDIR', 'ENAMETOOLONG', 'ENODATA', 'ENOENT', 'ENOLCK',
                'ENOMEM', 'ENOSYS', 'ENOTDIR', 'ENOTEMPTY', 'ENOTSUP',
                'ERANGE', 'EROFS'],
    'stat_h': ['UTIME_NOW', 'UTIME_OMIT'],
}
