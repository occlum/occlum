#ifndef __FUTEX_H_
#define __FUTEX_H_

#include <sys/time.h>

int futex_wait(volatile int* uaddr, int val);
int futex_wakeup(volatile int* uaddr);

#endif /* __ATOMIC_H_ */
