#include "ocalls.h"
#include <errno.h>
#include <unistd.h>
#include <sys/eventfd.h>
#include <sys/ioctl.h>

void occlum_ocall_sync(void) {
    sync();
}

int occlum_ocall_eventfd(unsigned int initval, int flags) {
    return eventfd(initval, flags);
}

int occlum_ocall_ioctl(int fd, int request, void *arg, size_t len) {
    if (((arg == NULL) ^ (len == 0)) == 1) {
        printf("invalid ioctl parameters\n");
        errno = EINVAL;
        return -1;
    }

    return ioctl(fd, request, arg);
}
