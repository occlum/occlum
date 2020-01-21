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
