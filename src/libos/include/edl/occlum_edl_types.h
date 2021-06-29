#ifndef __OCCLUM_EDL_TYPES_H__
#define __OCCLUM_EDL_TYPES_H__

#include <sys/struct_timespec.h>

typedef long                time_t;
typedef long                suseconds_t;
typedef long                syscall_slong_t;
typedef int                 clockid_t;

struct timeval {
    time_t      tv_sec;     /* seconds */
    suseconds_t tv_usec;    /* microseconds */
};

struct occlum_stdio_fds {
    int stdin_fd;
    int stdout_fd;
    int stderr_fd;
};

typedef struct _timespec{
    time_t tv_sec;
    syscall_slong_t tv_nsec;
};

typedef struct itimerspec{
    struct _timespec it_interval;
    struct _timespec it_value;
} itimerspec_t;

#define FD_SETSIZE 1024
typedef struct {
    unsigned long fds_bits[FD_SETSIZE / 8 / sizeof(long)];
} fd_set;

#endif /* __OCCLUM_EDL_TYPES_H__ */
