#define _GNU_SOURCE
#include <sys/types.h>
#include <sys/syscall.h>
#include <sys/time.h>
#include <unistd.h>
#include <pthread.h>
#include <stdio.h>
#include <linux/futex.h>
#include "test.h"

// ============================================================================
// Test case
// ============================================================================

//
// Three types of threads that will not exit voluntarily
//

// Type 1: a busy loop thread
static void *busyloop_thread_func(void *_) {
    while (1) {  }
    return NULL;
}

// Type 2: a sleeping thread
static void *sleeping_thread_func(void *_) {
    unsigned int a_year_in_sec = 365 * 24 * 60 * 60;
    sleep(a_year_in_sec);
    return NULL;
}

// Type 3: a thead that waits on a futex FOREVER
static void *futex_wait_thread_func(void *_) {
    int my_private_futex = 0;
    syscall(SYS_futex, &my_private_futex, FUTEX_WAIT, my_private_futex, NULL);
    return NULL;
}

// exit_group syscall should terminate all threads in a thread group.
int test_exit_group_to_force_threads_terminate(void) {
    // Create three types of threads that will not exit voluntarily
    pthread_t busyloop_thread;
    if (pthread_create(&busyloop_thread, NULL, busyloop_thread_func, NULL) < 0) {
        printf("ERROR: pthread_create failed\n");
        return -1;
    }
    pthread_t sleeping_thread;
    if (pthread_create(&sleeping_thread, NULL, sleeping_thread_func, NULL) < 0) {
        printf("ERROR: pthread_create failed\n");
        return -1;
    }
    pthread_t futex_wait_thread;
    if (pthread_create(&futex_wait_thread, NULL, futex_wait_thread_func, NULL) < 0) {
        printf("ERROR: pthread_create failed\n");
        return -1;
    }
    // Sleep for a while to make sure all three threads are running
    useconds_t half_second = 500 * 1000; // in us
    usleep(half_second);

    // exit_group syscall will be called eventually by libc's exit, after the
    // main function returns. If Occlum can terminate normally, this means
    // exit_group syscall taking effect.
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_exit_group_to_force_threads_terminate)
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
