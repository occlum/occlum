#ifndef __ATOMIC_H_
#define __ATOMIC_H_

static inline int a_load(int* n) {
    return *(volatile int*)n;
}

static inline int a_fetch_and_add(int* n, int a) {
    return __sync_fetch_and_add(n, a);
}

#endif /* __ATOMIC_H_ */
