#ifndef __OCCLUM_SYSCALL_H__
#define __OCCLUM_SYSCALL_H__

#include <sys/types.h>
#include "syscall_nr.h"

#ifdef __cplusplus
extern "C" {
#endif

extern int occlum_open(const char* path, int flags, int mode);
extern int occlum_close(int fd);
extern ssize_t occlum_read(int fd, void* buf, size_t size);
extern ssize_t occlum_write(int fd, const void* buf, size_t size);
extern int occlum_spawn(int* child_pid, const char* path,
                        const char** argv,
                        const char** envp);
extern int occlum_wait4(int child_pid, int* status, int options/*, struct rusage* rusage*/);
extern unsigned int occlum_getpid(void);
extern void occlum_exit(int status);

#ifdef __cplusplus
}
#endif

#endif /* __OCCLUM_SYSCALL_H__ */
