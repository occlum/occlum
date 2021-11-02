#include "ocalls.h"
#include <errno.h>
#include <net/if.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <sys/vfs.h>

void occlum_ocall_sync(void) {
    sync();
}

int occlum_ocall_ioctl(int fd, int request, void *arg, size_t len) {
    if (((arg == NULL) ^ (len == 0)) == 1) {
        errno = EINVAL;
        return -1;
    }

    return ioctl(fd, request, arg);
}

int occlum_ocall_statfs(const char *path, struct statfs *buf) {
    return statfs(path, buf);
}
