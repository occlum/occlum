#include "rusgx_stub.h"

static long __read(int fd, void* buf, unsigned long size) {
    long ret;
    __asm__ __volatile__ (
        "syscall"
        : "=a" (ret)
        : "0" (SYS_read), "D" (fd), "S" (buf), "d" (size)
        : "cc", "rcx", "r11", "memory"
    );
    return ret;
}

static long __write(int fd, const void* buf, unsigned long size) {
    long ret;
    __asm__ __volatile__ (
        "syscall"
        : "=a" (ret)
        : "0" (SYS_write), "D" (fd), "S" (buf), "d" (size)
        : "cc", "rcx", "r11", "memory"
    );
    return ret;
}

static long __open(const char* path, int flags, int mode) {
    long ret;
    __asm__ __volatile__ (
        "syscall"
        : "=a" (ret)
        : "0" (SYS_open), "D" (path), "S" (flags), "d" (mode)
        : "cc", "rcx", "r11", "memory"
    );
    return ret;
}

static long __close(int fd) {
    long ret;
    __asm__ __volatile__ (
        "syscall"
        : "=a" (ret)
        : "a" (SYS_close), "D" (fd)
        : "cc", "rcx", "r11", "memory" );
    return ret;
}

static void __exit(int status) {
    __asm__ __volatile__ (
        "syscall"
        :
        : "a" (SYS_exit), "D" (status)
        : "cc", "rcx", "r11", "memory" );
    return;
}

long rusgx_syscall(int num, long arg0, long arg1, long arg2, long arg3, long arg4) {
    long ret = 0;
    switch (num) {
    case SYS_exit:
        __exit((int)arg0);
        break;
    case SYS_read:
        ret = __read((int)arg0, (void*)arg1, (unsigned long)arg2);
        break;
    case SYS_write:
        ret = __write((int)arg0, (const void*)arg1, (unsigned long)arg2);
        break;
    case SYS_open:
        ret = __open((const char*)arg0, (int)arg1, (int)arg2);
        break;
    case SYS_close:
        ret = __close((int)arg0);
        break;
    default:
        break;
    }
    return ret;
}
