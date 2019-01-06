#include <stddef.h>
#include <sys/time.h>
#include <sys/syscall.h>
#include <limits.h>
#include <linux/futex.h>

static inline long __syscall6(long n, long a1, long a2, long a3, long a4, long a5, long a6)
{
    unsigned long ret;
    register long r10 __asm__("r10") = a4;
    register long r8 __asm__("r8") = a5;
    register long r9 __asm__("r9") = a6;
    __asm__ __volatile__ ("syscall" : "=a"(ret) : "a"(n), "D"(a1), "S"(a2),
                          "d"(a3), "r"(r10), "r"(r8), "r"(r9) : "rcx", "r11", "memory");
    return ret;
}

#define syscall(num, a1, a2, a3, a4, a5, a6) \
    __syscall6((num), (long)(a1), (long)(a2), (long)(a3), (long)(a4), (long)(a5), (long)(a6))

static inline int futex(volatile void *addr1, int op, int val1, struct timespec *timeout,
                void *addr2, int val3) {
    return (int) syscall(SYS_futex, addr1, op, val1, timeout, addr2, val3);
}

int futex_wait(volatile int* uaddr, int val) {
    return futex(uaddr, FUTEX_WAIT, val, NULL, NULL, 0);
}

int futex_wakeup(volatile int* uaddr) {
    return futex(uaddr, FUTEX_WAKE, INT_MAX, NULL, NULL, 0);
}
