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

#define SYS_exit    60
#define SYS_write   1

long rusgx_syscall(int num, long arg0, long arg1, long arg2, long arg3, long arg4);

static inline ssize_t __rusgx_write(int fd, const void* buf, unsigned long size) {
    return (ssize_t) rusgx_syscall(SYS_write, (long)fd, (long)buf, (long)size, (long)0, (long)0);
}

static inline void __rusgx_exit(int status) {
    rusgx_syscall(SYS_exit, (long)status, (long)0, (long)0, (long)0, (long)0);
}

#endif /* __RUSGX_STUB__ */
