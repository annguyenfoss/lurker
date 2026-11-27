/* Pure native timer contract test: no LKL kernel, filesystem or device. */
#include <assert.h>
#include <stdatomic.h>
#include <limits.h>
#include <unistd.h>
#include "netbsd-timer.h"
static atomic_uint hits, quiescing, drained;
static void *self_timer;
static pthread_mutex_t gate_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t gate_changed = PTHREAD_COND_INITIALIZER;
static int entered, release_callback;
static void delay_ms(unsigned milliseconds)
{
	struct timespec pause = {milliseconds / 1000, (long)(milliseconds % 1000) * 1000000};
	while (nanosleep(&pause, &pause) && errno == EINTR) {}
}
static void wait_count(atomic_uint *value, unsigned count)
{
	for (unsigned i = 0; i < 2000 && atomic_load(value) < count; i++) delay_ms(1);
	assert(atomic_load(value) >= count);
}
static void count(void) { atomic_fetch_add(&hits, 1); }
static void rearm(void)
{
	if (atomic_fetch_add(&hits, 1) == 0)
		assert(nb_lkl_timer_set_oneshot(self_timer, 1000000) == 0);
}
static void self_free(void)
{
	nb_lkl_timer_free(self_timer);
	atomic_fetch_add(&hits, 1);
}
static void blocked(void)
{
	assert(pthread_mutex_lock(&gate_lock) == 0);
	entered = 1;
	assert(pthread_cond_broadcast(&gate_changed) == 0);
	while (!release_callback) assert(pthread_cond_wait(&gate_changed, &gate_lock) == 0);
	assert(pthread_mutex_unlock(&gate_lock) == 0);
	atomic_fetch_add(&hits, 1);
}
static void *drain(void *unused)
{
	(void)unused;
	atomic_store(&quiescing, 1);
	nb_lkl_timers_quiesce();
	atomic_store(&drained, 1);
	return NULL;
}
int main(void)
{
	setvbuf(stdout, NULL, _IONBF, 0);
	assert(getuid() != 0 && geteuid() == getuid());
	printf("native timer control uid=%u\n", (unsigned)getuid());
	void *timer = nb_lkl_timer_alloc(count); assert(timer);
	assert(nb_lkl_timer_set_oneshot(timer, 1000000) == 0);
	wait_count(&hits, 1); delay_ms(20); assert(atomic_load(&hits) == 1);
	nb_lkl_timer_free(timer); nb_lkl_timers_quiesce();
	puts("one-shot and monotonic condition callback passed");

	atomic_store(&hits, 0); timer = nb_lkl_timer_alloc(count); assert(timer);
	assert(nb_lkl_timer_set_oneshot(timer, 100000000) == 0);
	assert(nb_lkl_timer_set_oneshot(timer, 1000000) == 0);
	wait_count(&hits, 1); delay_ms(150); assert(atomic_load(&hits) == 1);
	nb_lkl_timer_free(timer); nb_lkl_timers_quiesce();
	puts("replacement generation passed");

	atomic_store(&hits, 0); timer = nb_lkl_timer_alloc(count); assert(timer);
	assert(nb_lkl_timer_set_oneshot(timer, ULONG_MAX) == 0);
	delay_ms(10);
	assert(nb_lkl_timer_set_oneshot(timer, 0) == 0);
	delay_ms(20); assert(atomic_load(&hits) == 0);
	nb_lkl_timer_free(timer); nb_lkl_timers_quiesce();
	puts("large deadline and zero disarm passed");

	atomic_store(&hits, 0); self_timer = nb_lkl_timer_alloc(rearm); assert(self_timer);
	assert(nb_lkl_timer_set_oneshot(self_timer, 1000000) == 0);
	wait_count(&hits, 2); nb_lkl_timer_free(self_timer); nb_lkl_timers_quiesce();
	assert(atomic_load(&hits) == 2);
	puts("callback self-rearm passed");

	atomic_store(&hits, 0); self_timer = nb_lkl_timer_alloc(self_free); assert(self_timer);
	assert(nb_lkl_timer_set_oneshot(self_timer, 1000000) == 0);
	wait_count(&hits, 1); nb_lkl_timers_quiesce();
	assert(atomic_load(&hits) == 1);
	puts("callback self-free passed");

	atomic_store(&hits, 0); timer = nb_lkl_timer_alloc(blocked); assert(timer);
	assert(nb_lkl_timer_set_oneshot(timer, 1000000) == 0);
	assert(pthread_mutex_lock(&gate_lock) == 0);
	while (!entered) assert(pthread_cond_wait(&gate_changed, &gate_lock) == 0);
	assert(pthread_mutex_unlock(&gate_lock) == 0);
	/* The callback stays blocked. Free must not join it from this context. */
	nb_lkl_timer_free(timer);
	pthread_t waiter; assert(pthread_create(&waiter, NULL, drain, NULL) == 0);
	wait_count(&quiescing, 1); delay_ms(20); assert(!atomic_load(&drained));
	assert(pthread_mutex_lock(&gate_lock) == 0); release_callback = 1;
	assert(pthread_cond_broadcast(&gate_changed) == 0);
	assert(pthread_mutex_unlock(&gate_lock) == 0);
	assert(pthread_join(waiter, NULL) == 0);
	assert(atomic_load(&drained) && atomic_load(&hits) == 1);
	puts("nonblocking free and explicit callback quiescence passed");

	atomic_store(&hits, 0);
	for (unsigned i = 0; i < 32; i++) {
		timer = nb_lkl_timer_alloc(count); assert(timer);
		assert(nb_lkl_timer_set_oneshot(timer, 500000000) == 0);
		nb_lkl_timer_free(timer);
	}
	nb_lkl_timers_quiesce(); assert(!atomic_load(&hits));
	assert(nb_lkl_timers_live == 0);
	puts("32 cancelled workers drained; no callbacks after quiescence");
	return 0;
}
