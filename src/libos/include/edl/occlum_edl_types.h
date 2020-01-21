#ifndef __OCCLUM_EDL_TYPES_H__
#define __OCCLUM_EDL_TYPES_H__

typedef long                time_t;
typedef long                suseconds_t;
typedef int                 clockid_t;

struct timeval {
    time_t      tv_sec;     /* seconds */
    suseconds_t tv_usec;    /* microseconds */
};

#endif /* __OCCLUM_EDL_TYPES_H__ */
