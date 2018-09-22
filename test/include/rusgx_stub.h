#ifndef __RUSGX_STUB__
#define __RUSGX_STUB__

#include <stddef.h>
#include <sys/types.h>

/*
 * Stub for Rusgx syscalls
 *
 * Executables built with Rusgx's toolchain are dynamically linked with this
 * stub library. This stub library serves two purposes:
 *
 *  1) Enable Rusgx's syscalls. Since this library is dynamically linked with
 *  executables, the compile-time linker generates proper dynamic linking
 *  information. Using this information, the program loader of Rusgx can do
 *  runtime relocation so that user programs can make syscalls to the library
 *  OS.
 *
 *  2) Run without Rusgx. When not running upon Rusgx, the executables can use the
 *  host syscalls provided by the default implementation of this library.
 */

#define SYS_write   1
#define SYS_getpid  39
#define SYS_exit    60
#define SYS_wait4   61
#define SYS_spawn   360

long rusgx_syscall(int num, long arg0, long arg1, long arg2, long arg3, long arg4);

#define RUSGX_SYSCALL0(num) \
    rusgx_syscall((num), (long)0, (long)0, (long)0, (long)0, (long)0)
#define RUSGX_SYSCALL1(num, arg0) \
    rusgx_syscall((num), (long)(arg0), (long)0, (long)0, (long)0, (long)0)
#define RUSGX_SYSCALL2(num, arg0, arg1) \
    rusgx_syscall((num), (long)(arg0), (long)(arg1), (long)0, (long)0, (long)0)
#define RUSGX_SYSCALL3(num, arg0, arg1, arg2) \
    rusgx_syscall((num), (long)(arg0), (long)(arg1), (long)(arg2), (long)0, (long)0)
#define RUSGX_SYSCALL4(num, arg0, arg1, arg2, arg3) \
    rusgx_syscall((num), (long)(arg0), (long)(arg1), (long)(arg2), (long)(arg3), (long)0)
#define RUSGX_SYSCALL5(num, arg0, arg1, arg2, arg3, arg4) \
    rusgx_syscall((num), (long)(arg0), (long)(arg1), (long)(arg2), (long)(arg3), (long)(arg4))

static inline ssize_t __rusgx_write(int fd, const void* buf, unsigned long size) {
    return (ssize_t) RUSGX_SYSCALL3(SYS_write, fd, buf, size);
}

static inline unsigned int __rusgx_getpid(void) {
    return (unsigned int) RUSGX_SYSCALL0(SYS_getpid);
}

static inline void __rusgx_exit(int status) {
    RUSGX_SYSCALL1(SYS_exit, status);
}

static inline int __rusgx_spawn(int* child_pid, const char* path,
                        const char** argv, const char** envp) {
    return (int) RUSGX_SYSCALL4(SYS_spawn, child_pid, path, argv, envp);
}

static inline int __rusgx_wait4(int child_pid, int* status, int options/*, struct rusage* rusage*/) {
    return (int) RUSGX_SYSCALL3(SYS_wait4, child_pid, status, options);
}

#endif /* __RUSGX_STUB__ */
