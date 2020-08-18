#include "ocalls.h"
#include <errno.h>
#include <net/if.h>
#include <unistd.h>
#include <sys/eventfd.h>
#include <sys/ioctl.h>

void occlum_ocall_sync(void) {
    sync();
}

int occlum_ocall_eventfd(unsigned int initval, int flags) {
    return eventfd(initval, flags);
}

int occlum_ocall_ioctl_repack(int fd, int request, char *buf, int len, int *recv_len) {
    int ret = 0;

    switch (request) {
        case SIOCGIFCONF:
            if (recv_len == NULL) {
                errno = EINVAL;
                return -1;
            }

            struct ifconf config = { .ifc_len = len, .ifc_buf = buf };
            ret = ioctl(fd, SIOCGIFCONF, &config);
            if (ret == 0) {
                *recv_len = config.ifc_len;
            }
            break;

        default:
            errno = EINVAL;
            return -1;
    }

    return ret;
}

int occlum_ocall_ioctl(int fd, int request, void *arg, size_t len) {
    if (((arg == NULL) ^ (len == 0)) == 1) {
        errno = EINVAL;
        return -1;
    }

    return ioctl(fd, request, arg);
}
