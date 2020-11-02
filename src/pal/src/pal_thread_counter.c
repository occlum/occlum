#include <assert.h>
#include "pal_syscall.h"
#include "pal_thread_counter.h"

volatile int pal_thread_counter = 0;

void pal_thread_counter_inc(void) {
    __atomic_add_fetch(&pal_thread_counter, 1, __ATOMIC_SEQ_CST);
}

void pal_thread_counter_dec(void) {
    int val = __atomic_sub_fetch(&pal_thread_counter, 1, __ATOMIC_SEQ_CST);
    assert(val >= 0);

    (void)FUTEX_WAKE_ONE(&pal_thread_counter);
}

int pal_thread_counter_get(void) {
    return __atomic_load_n(&pal_thread_counter, __ATOMIC_SEQ_CST);
}

int pal_thread_counter_wait_zero(const struct timespec *timeout) {
    int old_val = pal_thread_counter_get();
    if (old_val == 0) { return 0; }

    (void)FUTEX_WAIT_TIMEOUT(&pal_thread_counter, old_val, timeout);

    int new_val = pal_thread_counter_get();
    return new_val;
}

