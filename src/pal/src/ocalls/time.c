#include <sys/time.h>
#include "ocalls.h"

void occlum_ocall_gettimeofday(long* seconds, long* microseconds) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    *seconds = tv.tv_sec;
    *microseconds = tv.tv_usec;
}

void occlum_ocall_clock_gettime(int clockid, time_t* sec, long* ns) {
    struct timespec ts;
    clock_gettime(clockid, &ts);
    *sec = ts.tv_sec;
    *ns = ts.tv_nsec;
}

void occlum_ocall_nanosleep(time_t sec, long nsec) {
    struct timespec tv = { .tv_sec = sec, .tv_nsec = nsec };
    nanosleep(&tv, NULL);
}
