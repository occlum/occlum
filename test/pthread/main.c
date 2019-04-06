#include <sys/types.h>
#include <pthread.h>
#include <stdio.h>

/*
 * Child threads
 */

#define NTHREADS            4
#define STACK_SIZE          (8 * 1024)

static void* thread_func(void* arg) {
    int* tid = arg;
    printf("tid = %d\n", *tid);
    return NULL;
}

int main(int argc, const char* argv[]) {
    pthread_t threads[NTHREADS];
    int thread_data[NTHREADS];

    printf("Creating %d threads...", NTHREADS);
    for (int ti = 0; ti < NTHREADS; ti++) {
        thread_data[ti] = ti;
        if (pthread_create(&threads[ti], NULL, thread_func, &thread_data[ti]) < 0) {
            printf("ERROR: pthread_create failed (ti = %d)\n", ti);
            return -1;
        }
    }
    printf("done.\n");

    printf("Waiting for %d threads to exit...", NTHREADS);
    for (int ti = 0; ti < NTHREADS; ti++) {
        if (pthread_join(threads[ti], NULL) < 0) {
            printf("ERROR: pthread_join failed (ti = %d)\n", ti);
            return -1;
        }
    }
    printf("done.\n");
    return 0;
}
