#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <unistd.h>
#include "../dragonfly-timer.h"
struct context { pthread_mutex_t lock; pthread_cond_t changed; void *timer; int calls; int mode; int entered; int released; int done; };
static void pause_ms(long ms) { struct timespec t = { ms / 1000, (ms % 1000)*1000000 }; while(nanosleep(&t,&t) && errno==EINTR) {} }
static void callback(void *opaque) {
 struct context *c=opaque;
 assert(!pthread_mutex_lock(&c->lock)); ++c->calls; c->entered=1; assert(!pthread_cond_broadcast(&c->changed));
 if(c->mode==3) { while(!c->released) assert(!pthread_cond_wait(&c->changed,&c->lock)); c->done=1; }
 int calls=c->calls; assert(!pthread_mutex_unlock(&c->lock));
 if(c->mode==1 && calls<5) assert(!df_timer_set_oneshot(c->timer,1000000));
 if(c->mode==2) df_timer_free(c->timer);
}
static void init(struct context *c,int mode) { *c=(struct context){.mode=mode}; assert(!pthread_mutex_init(&c->lock,NULL)); assert(!pthread_cond_init(&c->changed,NULL)); c->timer=df_timer_alloc(callback,c); assert(c->timer); }
static void wait_calls(struct context *c,int n) { struct timespec end; assert(!clock_gettime(CLOCK_REALTIME,&end)); end.tv_sec+=3; assert(!pthread_mutex_lock(&c->lock)); while(c->calls<n) assert(!pthread_cond_timedwait(&c->changed,&c->lock,&end)); assert(!pthread_mutex_unlock(&c->lock)); }
static void finish(struct context *c) { assert(!pthread_cond_destroy(&c->changed)); assert(!pthread_mutex_destroy(&c->lock)); }
static void *release_callback(void *opaque) { struct context *c=opaque; pause_ms(30); assert(!pthread_mutex_lock(&c->lock)); c->released=1; assert(!pthread_cond_broadcast(&c->changed)); assert(!pthread_mutex_unlock(&c->lock)); return NULL; }
int main(void) {
 assert(getuid()==1300 && geteuid()==1300); struct context c;
 init(&c,0); df_timer_free(c.timer); df_timers_quiesce(); assert(c.calls==0); finish(&c);
 init(&c,0); assert(!df_timer_set_oneshot(c.timer,120000000)); assert(!df_timer_set_oneshot(c.timer,5000000)); wait_calls(&c,1); pause_ms(150); df_timer_free(c.timer); df_timers_quiesce(); assert(c.calls==1); finish(&c);
 init(&c,1); assert(!df_timer_set_oneshot(c.timer,1000000)); wait_calls(&c,5); df_timer_free(c.timer); df_timers_quiesce(); assert(c.calls==5); finish(&c);
 init(&c,2); assert(!df_timer_set_oneshot(c.timer,1000000)); wait_calls(&c,1); df_timers_quiesce(); assert(c.calls==1); finish(&c);
 init(&c,3); assert(!df_timer_set_oneshot(c.timer,1000000)); wait_calls(&c,1); pthread_t release; assert(!pthread_create(&release,NULL,release_callback,&c)); df_timer_free(c.timer); df_timers_quiesce(); assert(c.done); assert(!pthread_join(release,NULL)); finish(&c);
 init(&c,0); assert(!df_timer_set_oneshot(c.timer,100000000)); assert(!df_timer_set_oneshot(c.timer,0)); pause_ms(120); df_timer_free(c.timer); df_timers_quiesce(); assert(c.calls==0); finish(&c);
 for(int i=0;i<100;i++) { init(&c,0); assert(!df_timer_set_oneshot(c.timer,100000000)); df_timer_free(c.timer); df_timers_quiesce(); assert(c.calls==0); finish(&c); }
 assert(df_timers_live==0); puts("native monotonic timer: arm/rearm, callback rearm/self-free, callback-drain, disarm and 100 cancel/free cycles passed"); return 0;
}
