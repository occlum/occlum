#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <sched.h>
#include <unistd.h>
#include <sys/syscall.h>

/*
 * Helper functions
 */

static inline int a_load(volatile int* x) {
    return __atomic_load_n((int*)x, __ATOMIC_SEQ_CST);
}

static inline int a_add_fetch(volatile int* x, int a) {
    return __atomic_add_fetch((int*)x, a, __ATOMIC_SEQ_CST);
}

/*
 * Futex wrapper
 */

#define FUTEX_NUM           202

#define FUTEX_WAIT          0
#define FUTEX_WAKE          1

// Libc does not provide a wrapper for futex, so we do it our own
static int futex(volatile int *futex_addr, int futex_op, int val) {
    return (int) syscall(FUTEX_NUM, futex_addr, futex_op, val);
}


/*
 * Child threads
 */

#define NTHREADS            4
#define STACK_SIZE          (8 * 1024)

volatile int num_exit_threads = 0;

static int thread_func(void* arg) {
    int* tid = arg;
    //printf("tid = %d\n", *tid);
    // Wake up the main thread if all child threads exit
    if (a_add_fetch(&num_exit_threads, 1) == NTHREADS) {
        futex(&num_exit_threads, FUTEX_WAKE, 1);
    }
    return 0;
}


int main(int argc, const char* argv[]) {
    unsigned int clone_flags = CLONE_VM | CLONE_FS | CLONE_FILES |
        CLONE_SIGHAND | CLONE_THREAD | CLONE_SYSVSEM | CLONE_DETACHED;

    printf("Creating %d threads...", NTHREADS);
    int thread_ids[NTHREADS];
    for (int tid = 0; tid < NTHREADS; tid++) {
        void* thread_stack = malloc(STACK_SIZE);
        if (thread_stack == NULL) {
            printf("ERROR: malloc failed for thread %d\n", tid);
            return -1;
        }

        thread_ids[tid] = tid;
        void* thread_arg = &thread_ids[tid];
        if (clone(thread_func, thread_stack, clone_flags, thread_arg) < 0) {
            printf("ERROR: clone failed for thread %d\n", tid);
            return -1;
        }
    }
    printf("done.\n");

    printf("Waiting for %d threads to exit...", NTHREADS);
    // Wait for all threads to exit
    int curr_num_exit_threads;
    while ((curr_num_exit_threads = a_load(&num_exit_threads)) != NTHREADS) {
        futex(&num_exit_threads, FUTEX_WAIT, curr_num_exit_threads);
    }
    printf("done.\n");

    return 0;
}
