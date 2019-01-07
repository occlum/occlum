#ifndef __OCCLUM_SYSCALL_H__
#define __OCCLUM_SYSCALL_H__

#include <sys/types.h>
#include "syscall_nr.h"

struct iovec;

#ifdef __cplusplus
extern "C" {
#endif

extern int occlum_open(const char* path, int flags, int mode);
extern int occlum_close(int fd);
extern ssize_t occlum_read(int fd, void* buf, size_t size);
extern ssize_t occlum_readv(int fd, struct iovec* iov, int count);
extern ssize_t occlum_write(int fd, const void* buf, size_t size);
extern ssize_t occlum_writev(int fd, const struct iovec* iov, int count);
extern off_t occlum_lseek(int fd, off_t offset, int whence);

extern int occlum_pipe(int fds[2]);
extern int occlum_pipe2(int fds[2], int flags);

extern int occlum_dup(int old_fd);
extern int occlum_dup2(int old_fd, int new_fd);
extern int occlum_dup3(int old_fd, int new_fd, int flags);

extern int occlum_spawn(int* child_pid, const char* path,
                        const char** argv, const char** envp,
                        void* file_actions);
extern int occlum_wait4(int child_pid, int* status, int options/*, struct rusage* rusage*/);
extern void occlum_exit(int status);
extern unsigned int occlum_getpid(void);
extern unsigned int occlum_getppid(void);

extern void *occlum_mmap(void *addr, size_t length, int prot, int flags, int fd, off_t offset);
extern int occlum_munmap(void *addr, size_t length);
extern void *occlum_mremap(void *old_address, size_t old_size, size_t new_size, int flags, void *new_address);
extern void* occlum_brk(void* addr);

extern int occlum_unknown(int num);

#ifdef __cplusplus
}
#endif

#endif /* __OCCLUM_SYSCALL_H__ */
