/* Native one-shot timer for the current LKL void-callback host contract.
 * Adapted from the project's reviewed NetBSD timer at 27341fd; no POSIX
 * SIGEV_THREAD dependency and no base kernel/library modification. */
#ifndef LKL_OPENBSD_TIMER_H
#define LKL_OPENBSD_TIMER_H
#include <errno.h>
#include <limits.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

_Static_assert(sizeof(time_t) == sizeof(int64_t), "OpenBSD amd64 time_t must be64-bit");

struct ob_lkl_timer {
	pthread_mutex_t lock;
	pthread_cond_t changed;
	unsigned refs;
	unsigned long generation;
	int armed;
	int stopping;
	struct timespec deadline;
	void (*callback)(void);
};
static pthread_mutex_t ob_lkl_timers_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ob_lkl_timers_done = PTHREAD_COND_INITIALIZER;
static unsigned long ob_lkl_timers_live;

static void ob_lkl_timer_check(const char *operation, int error)
{
	if (error) {
		fprintf(stderr, "OpenBSD LKL timer %s failed: %d\n", operation, error);
		abort();
	}
}
static void ob_lkl_timer_unref(struct ob_lkl_timer *timer)
{
	if (__sync_fetch_and_sub(&timer->refs, 1) == 1) {
		ob_lkl_timer_check("condition destruction", pthread_cond_destroy(&timer->changed));
		ob_lkl_timer_check("mutex destruction", pthread_mutex_destroy(&timer->lock));
		free(timer);
	}
}
static void ob_lkl_timer_worker_finished(void)
{
	ob_lkl_timer_check("registry lock", pthread_mutex_lock(&ob_lkl_timers_lock));
	if (!ob_lkl_timers_live) abort();
	--ob_lkl_timers_live;
	ob_lkl_timer_check("registry signal", pthread_cond_broadcast(&ob_lkl_timers_done));
	ob_lkl_timer_check("registry unlock", pthread_mutex_unlock(&ob_lkl_timers_lock));
}
static void *ob_lkl_timer_worker(void *opaque)
{
	struct ob_lkl_timer *timer = opaque;
	ob_lkl_timer_check("worker lock", pthread_mutex_lock(&timer->lock));
	while (!timer->stopping) {
		struct timespec deadline;
		unsigned long generation;
		int error;
		while (!timer->armed && !timer->stopping)
			ob_lkl_timer_check("wait", pthread_cond_wait(&timer->changed, &timer->lock));
		if (timer->stopping) break;
		deadline = timer->deadline;
		generation = timer->generation;
		error = pthread_cond_timedwait(&timer->changed, &timer->lock, &deadline);
		if (timer->stopping) break;
		if (error && error != ETIMEDOUT) ob_lkl_timer_check("timed wait", error);
		if (error != ETIMEDOUT || !timer->armed || generation != timer->generation)
			continue;
		timer->armed = 0;
		ob_lkl_timer_check("callback unlock", pthread_mutex_unlock(&timer->lock));
		timer->callback();
		ob_lkl_timer_check("callback relock", pthread_mutex_lock(&timer->lock));
	}
	ob_lkl_timer_check("worker unlock", pthread_mutex_unlock(&timer->lock));
	ob_lkl_timer_unref(timer);
	/* No LKL callback or timer-object access occurs after this marker. */
	ob_lkl_timer_worker_finished();
	return NULL;
}
static void *ob_lkl_timer_alloc(void (*callback)(void))
{
	struct ob_lkl_timer *timer;
	pthread_condattr_t condition_attr;
	pthread_attr_t thread_attr;
	pthread_t worker;
	int error, have_mutex = 0, have_attr = 0, have_condition = 0, have_thread_attr = 0;
	if (!callback) { errno = EINVAL; return NULL; }
	timer = calloc(1, sizeof(*timer));
	if (!timer) return NULL;
	timer->callback = callback;
	timer->refs = 2; /* One external handle and the worker lifetime. */
	if ((error = pthread_mutex_init(&timer->lock, NULL))) goto fail;
	have_mutex = 1;
	if ((error = pthread_condattr_init(&condition_attr))) goto fail;
	have_attr = 1;
	if ((error = pthread_condattr_setclock(&condition_attr, CLOCK_MONOTONIC))) goto fail;
	if ((error = pthread_cond_init(&timer->changed, &condition_attr))) goto fail;
	have_condition = 1;
	if ((error = pthread_attr_init(&thread_attr))) goto fail;
	have_thread_attr = 1;
	if ((error = pthread_attr_setdetachstate(&thread_attr, PTHREAD_CREATE_DETACHED))) goto fail;
	ob_lkl_timer_check("registry lock", pthread_mutex_lock(&ob_lkl_timers_lock));
	++ob_lkl_timers_live;
	ob_lkl_timer_check("registry unlock", pthread_mutex_unlock(&ob_lkl_timers_lock));
	error = pthread_create(&worker, &thread_attr, ob_lkl_timer_worker, timer);
	if (error) { ob_lkl_timer_worker_finished(); goto fail; }
	ob_lkl_timer_check("attribute destruction", pthread_attr_destroy(&thread_attr));
	ob_lkl_timer_check("condition attribute destruction", pthread_condattr_destroy(&condition_attr));
	return timer;
fail:
	if (have_thread_attr) pthread_attr_destroy(&thread_attr);
	if (have_condition) pthread_cond_destroy(&timer->changed);
	if (have_attr) pthread_condattr_destroy(&condition_attr);
	if (have_mutex) pthread_mutex_destroy(&timer->lock);
	free(timer);
	errno = error;
	return NULL;
}
static int ob_lkl_timer_set_oneshot(void *opaque, unsigned long nanoseconds)
{
	struct ob_lkl_timer *timer = opaque;
	struct timespec deadline;
	if (clock_gettime(CLOCK_MONOTONIC, &deadline)) return -1;
	if (deadline.tv_sec < 0 || deadline.tv_nsec < 0 || deadline.tv_nsec >= 1000000000L ||
	    (uint64_t)deadline.tv_sec > (uint64_t)INT64_MAX - nanoseconds / 1000000000UL - 1) {
		errno = EOVERFLOW;
		return -1;
	}
	deadline.tv_sec += nanoseconds / 1000000000UL;
	deadline.tv_nsec += nanoseconds % 1000000000UL;
	if (deadline.tv_nsec >= 1000000000L) { ++deadline.tv_sec; deadline.tv_nsec -= 1000000000L; }
	ob_lkl_timer_check("arm lock", pthread_mutex_lock(&timer->lock));
	if (timer->stopping) {
		ob_lkl_timer_check("stopped arm unlock", pthread_mutex_unlock(&timer->lock));
		errno = EINVAL;
		return -1;
	}
	timer->deadline = deadline;
	timer->armed = nanoseconds != 0;
	++timer->generation;
	ob_lkl_timer_check("arm signal", pthread_cond_signal(&timer->changed));
	ob_lkl_timer_check("arm unlock", pthread_mutex_unlock(&timer->lock));
	return 0;
}
/* Only after LKL CPU/thread/tick shutdown, before freeing LKL memory.
 * Runtime timer_free must remain nonblocking even outside the timer thread:
 * its callback may be waiting for CPU/IRQ ownership held by the cancelling task.
 */
static void ob_lkl_timers_quiesce(void)
{
	ob_lkl_timer_check("drain lock", pthread_mutex_lock(&ob_lkl_timers_lock));
	while (ob_lkl_timers_live)
		ob_lkl_timer_check("drain wait", pthread_cond_wait(&ob_lkl_timers_done, &ob_lkl_timers_lock));
	ob_lkl_timer_check("drain unlock", pthread_mutex_unlock(&ob_lkl_timers_lock));
}
static void ob_lkl_timer_free(void *opaque)
{
	struct ob_lkl_timer *timer = opaque;
	/* Runtime clock-event cancellation may originate inside this callback. */
	ob_lkl_timer_check("cancel lock", pthread_mutex_lock(&timer->lock));
	timer->stopping = 1;
	timer->armed = 0;
	ob_lkl_timer_check("cancel signal", pthread_cond_signal(&timer->changed));
	ob_lkl_timer_check("cancel unlock", pthread_mutex_unlock(&timer->lock));
	ob_lkl_timer_unref(timer);
}
#endif
