/* Pure native stack/mmap contract controls. No LKL library or filesystem mount. */
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <pthread_np.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
static void *last_mapping, *last_unmapping;
static unsigned mapping_calls;
static void *observed_mmap(void *addr, size_t size, int prot, int flags, int fd, off_t offset)
{
	++mapping_calls;
	return last_mapping = mmap(addr, size, prot, flags, fd, offset);
}
static int observed_munmap(void *addr, size_t size)
{
	last_unmapping = addr;
	return munmap(addr, size);
}
#define mmap observed_mmap
#define munmap observed_munmap
#include "openbsd-memory.h"
#undef mmap
#undef munmap
int main(void)
{
	setvbuf(stdout, NULL, _IONBF, 0);
	assert(getuid() != 0 && geteuid() == getuid());
	unsigned long stack_size = 0;
	void *stack = ob_lkl_thread_stack(&stack_size);
	assert(stack && stack_size);
	uintptr_t local = (uintptr_t)&stack_size;
	assert(local >= (uintptr_t)stack && local - (uintptr_t)stack < stack_size);
	stack_t native;
	assert(pthread_stackseg_np(pthread_self(), &native) == 0);
	assert((uintptr_t)stack + stack_size == (uintptr_t)native.ss_sp);
	puts("native stack top converted to checked lower bound passed");

	size_t page = (size_t)sysconf(_SC_PAGESIZE);
	unsigned char *original = mmap(NULL, page, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANON, -1, 0);
	assert(original != MAP_FAILED);
	memset(original, 0xa5, page);
	last_unmapping = NULL;
	assert(ob_lkl_mmap_exact(original, page, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANON, -1, 0) == NULL);
	assert(errno == EEXIST);
	assert(last_mapping != MAP_FAILED && last_mapping != original);
	assert(last_unmapping == last_mapping && last_unmapping != original);
	/* Read-only availability query checks cleanup, never selects a mapping. */
	assert(mquery(last_unmapping, page, PROT_READ|PROT_WRITE,
	              MAP_FIXED|MAP_ANON|MAP_PRIVATE, -1, 0) == last_unmapping);
	assert(mquery(original, page, PROT_READ|PROT_WRITE,
	              MAP_FIXED|MAP_ANON|MAP_PRIVATE, -1, 0) == MAP_FAILED);
	for (size_t i = 0; i < page; ++i) assert(original[i] == 0xa5);
	puts("occupied hint preserved; only returned alternate mapping released passed");

	unsigned calls = mapping_calls;
	assert(!ob_lkl_mmap_exact(original, page, PROT_READ, MAP_PRIVATE|MAP_ANON|MAP_FIXED, -1, 0));
	assert(!ob_lkl_mmap_exact(original+1, page, PROT_READ, MAP_PRIVATE|MAP_ANON, -1, 0));
	assert(!ob_lkl_mmap_exact(original, SIZE_MAX, PROT_READ, MAP_PRIVATE|MAP_ANON, -1, 0));
	assert(!ob_lkl_mmap_exact(original, page, PROT_READ, MAP_PRIVATE|MAP_SHARED, -1, 0));
	assert(!ob_lkl_mmap_exact(original, page, PROT_READ, MAP_SHARED, 0, 1));
	assert(mapping_calls == calls);
	for (size_t i = 0; i < page; ++i) assert(original[i] == 0xa5);
	puts("fixed/unaligned/overflow/invalid mapping requests refused before syscall passed");

	last_unmapping = NULL;
	assert(!ob_lkl_mmap_exact(original, page, PROT_READ, MAP_SHARED, -1, 0));
	assert(last_mapping == MAP_FAILED && last_unmapping == NULL);
	assert(errno == EBADF);
	for (size_t i = 0; i < page; ++i) assert(original[i] == 0xa5);
	puts("failed syscall does not unmap occupied hint passed");

	assert(munmap(original, page) == 0);
	void *exact = ob_lkl_mmap_exact(original, page, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANON, -1, 0);
	assert(exact == original);
	memset(exact, 0x5a, page);
	assert(munmap(exact, page) == 0);
	puts("anonymous exact hint reuse passed");

	char path[] = "/tmp/lurker-openbsd-lkl-memory-XXXXXXXX";
	int fd = mkstemp(path); assert(fd >= 0);
	assert(unlink(path) == 0 && ftruncate(fd, (off_t)page) == 0);
	void *hint = mmap(NULL, page, PROT_READ|PROT_WRITE, MAP_SHARED, fd, 0);
	assert(hint != MAP_FAILED && munmap(hint, page) == 0);
	exact = ob_lkl_mmap_exact(hint, page, PROT_READ|PROT_WRITE, MAP_SHARED, fd, 0);
	if (exact) {
		assert(exact == hint);
		memcpy(exact, "native shared mapping", 21);
		assert(msync(exact, page, MS_SYNC) == 0 && munmap(exact, page) == 0);
		char bytes[21]; assert(pread(fd, bytes, sizeof(bytes), 0) == sizeof(bytes));
		assert(memcmp(bytes, "native shared mapping", sizeof(bytes)) == 0);
		puts("shared file exact hint and native contents passed");
	} else {
		assert(errno == EEXIST);
		puts("shared file exact hint safely rejected by native mapping policy");
	}
	assert(close(fd) == 0);
	return 0;
}
