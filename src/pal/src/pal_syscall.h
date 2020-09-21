#ifndef __PAL_SYSCALL_H__
#define __PAL_SYSCALL_H__

#define _GNU_SOURCE
#include <linux/futex.h>
#include <sys/time.h>
#include <sys/types.h>
#include <sys/syscall.h>
#include <unistd.h>

#define gettid()                        ((pid_t)syscall(__NR_gettid))
#define tgkill(tgid, tid, signum)       ((int)syscall(__NR_tgkill, (tgid), (tid), (signum)));
#define futex_wait(addr, val, timeout)  ((int)syscall(__NR_futex, (addr), FUTEX_WAIT, (val), (timeout)))
#define futex_wake(addr)                ((int)syscall(__NR_futex, (addr), FUTEX_WAKE, 1))
#define raw_ppoll(fds, nfds, timeout)   ((int)syscall(__NR_ppoll, (fds), (nfds), (timeout), NULL, 0))

#endif /* __PAL_SYSCALL_H__ */
