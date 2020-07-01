#ifndef __PAL_THREAD_COUNTER_H__
#define __PAL_THREAD_COUNTER_H__

#include <time.h>

// An atomic counter for threads

// Increase the counter atomically
void pal_thread_counter_inc(void);

// Decrease the counter atomically. Don't try to decrease the value below zero.
void pal_thread_counter_dec(void);

// Get the value of the counter atomically
int pal_thread_counter_get(void);

// Wait for counter to be zero until a timeout
int pal_thread_counter_wait_zero(const struct timespec *timeout);

#endif /* __PAL_THREAD_COUNTER_H__ */
