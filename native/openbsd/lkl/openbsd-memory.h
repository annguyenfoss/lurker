/* OpenBSD amd64 primitives for a future LKL POSIX host adapter. No MAP_FIXED. */
#ifndef LKL_OPENBSD_MEMORY_H
#define LKL_OPENBSD_MEMORY_H
#include <errno.h>
#include <pthread.h>
#include <pthread_np.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <unistd.h>

_Static_assert(sizeof(off_t) == sizeof(int64_t), "OpenBSD amd64 off_t must be 64-bit");

static void *ob_lkl_thread_stack(unsigned long *size)
{
	stack_t info;
	uintptr_t top, base;
	int error = pthread_stackseg_np(pthread_self(), &info);
	if (error) { errno = error; return NULL; }
	top = (uintptr_t)info.ss_sp;
	if (!info.ss_size || top < info.ss_size) { errno = EOVERFLOW; return NULL; }
	base = top - info.ss_size;
	if (!base) { errno = EINVAL; return NULL; }
	if (size) *size = info.ss_size;
	return (void *)base;
}

static void *ob_lkl_mmap_exact(void *addr, size_t size, int prot, int flags,
                             int fd, off_t offset)
{
	long page = sysconf(_SC_PAGESIZE);
	size_t rounded;
	void *result;
	if (page <= 0 || !addr || !size || (uintptr_t)addr % (size_t)page ||
	    offset < 0 || (uint64_t)offset % (size_t)page ||
	    size > SIZE_MAX - ((size_t)page - 1) || (flags & MAP_FIXED) ||
	    (!(flags & MAP_PRIVATE) == !(flags & MAP_SHARED)) ||
	    ((flags & MAP_ANON) && (fd != -1 || offset != 0))) {
		errno = EINVAL;
		return NULL;
	}
	rounded = (size + (size_t)page - 1) / (size_t)page * (size_t)page;
	if ((uintptr_t)addr > UINTPTR_MAX - rounded ||
	    rounded > (uint64_t)INT64_MAX || (uint64_t)offset > (uint64_t)INT64_MAX - rounded) {
		errno = EOVERFLOW;
		return NULL;
	}
	result = mmap(addr, size, prot, flags, fd, offset);
	if (result == MAP_FAILED) return NULL;
	if (result != addr) {
		/* Release only the mapping this invocation actually acquired. */
		if (munmap(result, size)) abort();
		errno = EEXIST;
		return NULL;
	}
	return result;
}
#endif
