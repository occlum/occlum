#include <sys/types.h>
#include <pthread.h>
#include <stdio.h>

/*
 * Child threads
 */

#define NTHREADS                (4)
#define STACK_SIZE              (8 * 1024)

#define LOCAL_COUNT             (100000L)
#define EXPECTED_GLOBAL_COUNT   (LOCAL_COUNT * NTHREADS)

struct thread_arg {
    int                         ti;
    long                        local_count;
    volatile long*              global_count;
    pthread_mutex_t*            mutex;
};

static void* thread_func(void* _arg) {
    struct thread_arg* arg = _arg;
    printf("Thread #%d: started\n", arg->ti);
    for (long i = 0; i < arg->local_count; i++) {
        pthread_mutex_lock(arg->mutex);
        (*arg->global_count)++;
        pthread_mutex_unlock(arg->mutex);
    }
    printf("Thread #%d: completed\n", arg->ti);
    return NULL;
}

int main(int argc, const char* argv[]) {
    /*
     * Multiple threads are to increase a global counter concurrently
     */
    volatile long global_count = 0;
    pthread_t threads[NTHREADS];
    struct thread_arg thread_args[NTHREADS];
    /*
     * Protect the counter with a mutex
     */
    pthread_mutex_t mutex;
    pthread_mutex_init(&mutex, NULL);
    /*
     * Start the threads
     */
    for (int ti = 0; ti < NTHREADS; ti++) {
        struct thread_arg* thread_arg = &thread_args[ti];
        thread_arg->ti = ti;
        thread_arg->local_count = LOCAL_COUNT;
        thread_arg->global_count = &global_count;
        thread_arg->mutex = &mutex;

        if (pthread_create(&threads[ti], NULL, thread_func, thread_arg) < 0) {
            printf("ERROR: pthread_create failed (ti = %d)\n", ti);
            return -1;
        }
    }
    /*
     * Wait for the threads to finish
     */
    for (int ti = 0; ti < NTHREADS; ti++) {
        if (pthread_join(threads[ti], NULL) < 0) {
            printf("ERROR: pthread_join failed (ti = %d)\n", ti);
            return -1;
        }
    }
    /*
     * Check the correctness of the concurrent counter
     */
    if (global_count != EXPECTED_GLOBAL_COUNT) {
        printf("ERROR: incorrect global_count (actual = %ld, expected = %ld)\n",
                global_count, EXPECTED_GLOBAL_COUNT);
        return -1;
    }

    pthread_mutex_destroy(&mutex);
    return 0;
}
