#include <pthread.h>
#include <sys/time.h>
#include "ocalls.h"

void occlum_ocall_gettimeofday(struct timeval* tv) {
    gettimeofday(tv, NULL);
}

void occlum_ocall_clock_gettime(int clockid, struct timespec *tp) {
    clock_gettime(clockid, tp);
}

void occlum_ocall_nanosleep(const struct timespec* req) {
    nanosleep(req, NULL);
}

int occlum_ocall_thread_getcpuclock(struct timespec *tp) {
    clockid_t thread_clock_id;
    int ret = pthread_getcpuclockid(pthread_self(), &thread_clock_id);
    if(ret != 0) {
       PAL_ERROR("failed to get clock id");
       return -1;
    }

    return clock_gettime(thread_clock_id, tp);
}
