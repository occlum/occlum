#include <errno.h>
#include "errno2str.h"

const char *errno2str(int errno_) {
    switch (errno_) {
        case EPERM:
            return "EPERM";
        case ENOENT:
            return "ENOENT";
        case ESRCH:
            return "ESRCH";
        case ENOEXEC:
            return "ENOEXEC";
        case EBADF:
            return "EBADF";
        case ECHILD:
            return "ECHILD";
        case EAGAIN:
            return "EAGAIN";
        case ENOMEM:
            return "ENOMEM";
        case EACCES:
            return "EACCES";
        case EFAULT:
            return "EFAULT";
        case EBUSY:
            return "EBUSY";
        case EINVAL:
            return "EINVAL";
        case ENOSYS:
            return "ENOSYS";
        default:
            return "unknown";
    }
}
