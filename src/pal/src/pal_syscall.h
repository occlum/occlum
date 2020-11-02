#ifndef __PAL_SYSCALL_H__
#define __PAL_SYSCALL_H__

#define _GNU_SOURCE
#include <linux/futex.h>
#include <sys/time.h>
#include <sys/types.h>
#include <sys/syscall.h>
#include <unistd.h>

#define GETTID()                        ((pid_t)syscall(__NR_gettid))
#define TGKILL(tgid, tid, signum)       ((int)syscall(__NR_tgkill, (tgid), (tid), (signum)))
#define FUTEX_WAIT_TIMEOUT(addr, val, timeout)  ((int)syscall(__NR_futex, (addr), FUTEX_WAIT, (val), (timeout)))
#define FUTEX_WAKE_ONE(addr)                ((int)syscall(__NR_futex, (addr), FUTEX_WAKE, 1))
#define RAW_PPOLL(fds, nfds, timeout)   ((int)syscall(__NR_ppoll, (fds), (nfds), (timeout), NULL, 0))

#endif /* __PAL_SYSCALL_H__ */
