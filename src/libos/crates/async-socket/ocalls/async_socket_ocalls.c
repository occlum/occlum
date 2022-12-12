#include <errno.h>
#include <net/if.h>
#include <unistd.h>
#include <sys/ioctl.h>

int socket_ocall_ioctl_repack(int fd, int request, char *buf, int len, int *recv_len) {
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