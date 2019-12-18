#ifndef __PAL_SYSCALL_H__
#define __PAL_SYSCALL_H__

#define _GNU_SOURCE
#include <sys/syscall.h>
#include <unistd.h>

#define gettid() syscall(__NR_gettid)

#endif /* __PAL_SYSCALL_H__ */
