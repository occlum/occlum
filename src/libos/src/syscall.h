#ifndef __RUSGX_SYSCALL_H__
#define __RUSGX_SYSCALL_H__

#include <sys/types.h>

#define SYS_exit            60
#define SYS_write           1

#ifdef __cplusplus
extern "C" {
#endif

extern ssize_t rusgx_write(int fd, const void* buf, size_t size);

#ifdef __cplusplus
}
#endif

#endif /* __RUSGX_SYSCALL_H__ */
