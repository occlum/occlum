#include <linux/futex.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <errno.h>
#include <stdio.h>
#include <time.h>
#include <stdint.h>

int ocall_futex_wait_timeout(int32_t *err, uint32_t *uaddr, struct timespec *timeout, uint32_t val) {
    int ret = syscall(SYS_futex, uaddr, FUTEX_WAIT, val, timeout, NULL, 0);
    *err = errno;
    return ret;
}

int ocall_futex_wake(int32_t *err, uint32_t *uaddr) {
    int ret = syscall(SYS_futex, uaddr, FUTEX_WAKE, 1, NULL, NULL, 0);
    *err = errno;
    return ret;
}
