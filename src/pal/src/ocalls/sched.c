#include <sched.h>
#include "ocalls.h"

int occlum_ocall_sched_getaffinity(int* error, int pid, size_t cpusize, unsigned char* buf) {
    int ret = syscall(__NR_sched_getaffinity, pid, cpusize, buf);
    if (error) {
        *error = (ret == -1) ? errno : 0;
    }
    return ret;
}

int occlum_ocall_sched_setaffinity(int* error, int pid, size_t cpusize, const unsigned char* buf) {
    int ret = syscall(__NR_sched_setaffinity, pid, cpusize, buf);
    if (error) {
        *error = (ret == -1) ? errno : 0;
    }
    return ret;
}

/* In the Linux implementation, sched_yield() always succeeds */
void occlum_ocall_sched_yield(void) {
    sched_yield();
}

