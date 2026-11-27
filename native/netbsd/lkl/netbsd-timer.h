/* Native one-shot timer for the current LKL void-callback host contract.
 * Adapted from the project's DragonFly timer at fbdfbc5; no POSIX
 * SIGEV_THREAD dependency and no base kernel/library modification. */
#ifndef LKL_NETBSD_TIMER_H
#define LKL_NETBSD_TIMER_H
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

struct nb_lkl_timer {
	pthread_mutex_t lock;
	pthread_cond_t changed;
	unsigned refs;
	unsigned long generation;
	int armed;
	int stopping;
	struct timespec deadline;
	void (*callback)(void);
};
static pthread_mutex_t nb_lkl_timers_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t nb_lkl_timers_done = PTHREAD_COND_INITIALIZER;
static unsigned long nb_lkl_timers_live;

static void nb_lkl_timer_check(const char *operation, int error)
{
	if (error) {
		fprintf(stderr, "NetBSD LKL timer %s failed: %d\n", operation, error);
		abort();
	}
}
static void nb_lkl_timer_unref(struct nb_lkl_timer *timer)
{
	if (__sync_fetch_and_sub(&timer->refs, 1) == 1) {
		nb_lkl_timer_check("condition destruction", pthread_cond_destroy(&timer->changed));
		nb_lkl_timer_check("mutex destruction", pthread_mutex_destroy(&timer->lock));
		free(timer);
	}
}
static void nb_lkl_timer_worker_finished(void)
{
	nb_lkl_timer_check("registry lock", pthread_mutex_lock(&nb_lkl_timers_lock));
	if (!nb_lkl_timers_live) abort();
	--nb_lkl_timers_live;
	nb_lkl_timer_check("registry signal", pthread_cond_broadcast(&nb_lkl_timers_done));
	nb_lkl_timer_check("registry unlock", pthread_mutex_unlock(&nb_lkl_timers_lock));
}
static void *nb_lkl_timer_worker(void *opaque)
{
	struct nb_lkl_timer *timer = opaque;
	nb_lkl_timer_check("worker lock", pthread_mutex_lock(&timer->lock));
	while (!timer->stopping) {
		struct timespec deadline;
		unsigned long generation;
		int error;
		while (!timer->armed && !timer->stopping)
			nb_lkl_timer_check("wait", pthread_cond_wait(&timer->changed, &timer->lock));
		if (timer->stopping) break;
		deadline = timer->deadline;
		generation = timer->generation;
		error = pthread_cond_timedwait(&timer->changed, &timer->lock, &deadline);
		if (timer->stopping) break;
		if (error && error != ETIMEDOUT) nb_lkl_timer_check("timed wait", error);
		if (error != ETIMEDOUT || !timer->armed || generation != timer->generation)
			continue;
		timer->armed = 0;
		nb_lkl_timer_check("callback unlock", pthread_mutex_unlock(&timer->lock));
		timer->callback();
		nb_lkl_timer_check("callback relock", pthread_mutex_lock(&timer->lock));
	}
	nb_lkl_timer_check("worker unlock", pthread_mutex_unlock(&timer->lock));
	nb_lkl_timer_unref(timer);
	/* No LKL callback or timer-object access occurs after this marker. */
	nb_lkl_timer_worker_finished();
	return NULL;
}
static void *nb_lkl_timer_alloc(void (*callback)(void))
{
	struct nb_lkl_timer *timer;
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
	nb_lkl_timer_check("registry lock", pthread_mutex_lock(&nb_lkl_timers_lock));
	++nb_lkl_timers_live;
	nb_lkl_timer_check("registry unlock", pthread_mutex_unlock(&nb_lkl_timers_lock));
	error = pthread_create(&worker, &thread_attr, nb_lkl_timer_worker, timer);
	if (error) { nb_lkl_timer_worker_finished(); goto fail; }
	nb_lkl_timer_check("attribute destruction", pthread_attr_destroy(&thread_attr));
	nb_lkl_timer_check("condition attribute destruction", pthread_condattr_destroy(&condition_attr));
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
static int nb_lkl_timer_set_oneshot(void *opaque, unsigned long nanoseconds)
{
	struct nb_lkl_timer *timer = opaque;
	struct timespec deadline;
	if (clock_gettime(CLOCK_MONOTONIC, &deadline)) return -1;
	deadline.tv_sec += nanoseconds / 1000000000UL;
	deadline.tv_nsec += nanoseconds % 1000000000UL;
	if (deadline.tv_nsec >= 1000000000L) { ++deadline.tv_sec; deadline.tv_nsec -= 1000000000L; }
	nb_lkl_timer_check("arm lock", pthread_mutex_lock(&timer->lock));
	if (timer->stopping) {
		nb_lkl_timer_check("stopped arm unlock", pthread_mutex_unlock(&timer->lock));
		errno = EINVAL;
		return -1;
	}
	timer->deadline = deadline;
	timer->armed = nanoseconds != 0;
	++timer->generation;
	nb_lkl_timer_check("arm signal", pthread_cond_signal(&timer->changed));
	nb_lkl_timer_check("arm unlock", pthread_mutex_unlock(&timer->lock));
	return 0;
}
static void nb_lkl_timers_quiesce(void)
{
	nb_lkl_timer_check("drain lock", pthread_mutex_lock(&nb_lkl_timers_lock));
	while (nb_lkl_timers_live)
		nb_lkl_timer_check("drain wait", pthread_cond_wait(&nb_lkl_timers_done, &nb_lkl_timers_lock));
	nb_lkl_timer_check("drain unlock", pthread_mutex_unlock(&nb_lkl_timers_lock));
}
static void nb_lkl_timer_free(void *opaque)
{
	struct nb_lkl_timer *timer = opaque;
	/* Runtime clock-event cancellation may originate inside this callback. */
	nb_lkl_timer_check("cancel lock", pthread_mutex_lock(&timer->lock));
	timer->stopping = 1;
	timer->armed = 0;
	nb_lkl_timer_check("cancel signal", pthread_cond_signal(&timer->changed));
	nb_lkl_timer_check("cancel unlock", pthread_mutex_unlock(&timer->lock));
	nb_lkl_timer_unref(timer);
}
#endif
