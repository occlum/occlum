#ifndef __RUSGX_SYSCALL_H__
#define __RUSGX_SYSCALL_H__

#include <sys/types.h>
#include "syscall_nr.h"

#ifdef __cplusplus
extern "C" {
#endif

extern int rusgx_open(const char* path, int flags, int mode);
extern int rusgx_close(int fd);
extern ssize_t rusgx_read(int fd, void* buf, size_t size);
extern ssize_t rusgx_write(int fd, const void* buf, size_t size);
extern int rusgx_spawn(int* child_pid, const char* path,
                        const char** argv,
                        const char** envp);
extern int rusgx_wait4(int child_pid, int* status, int options/*, struct rusage* rusage*/);
extern unsigned int rusgx_getpid(void);
extern void rusgx_exit(int status);

#ifdef __cplusplus
}
#endif

#endif /* __RUSGX_SYSCALL_H__ */
