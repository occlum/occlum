#include <stdio.h>
#include <stdlib.h>
#define _GNU_SOURCE
#include <sched.h>

#define NTHREADS        4
#define STACK_SIZE      (8 * 1024)

// From file arch/x86_64/atomic_arch.h in musl libc. MIT License.
static inline void a_inc(volatile int *p)
{
    __asm__ __volatile__(
        "lock ; incl %0"
        : "=m"(*p) : "m"(*p) : "memory" );
}

volatile int num_exit_threads = 0;

int thread_func(void* arg) {
    int* tid = arg;
    //printf("tid = %d\n", *tid);
    a_inc(&num_exit_threads);
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
    while (num_exit_threads != NTHREADS);
    printf("done.\n");

    return 0;
}
