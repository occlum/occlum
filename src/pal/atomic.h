#ifndef __ATOMIC_H_
#define __ATOMIC_H_

static inline int a_load(volatile int* n) {
    return *(volatile int*)n;
}

static inline void a_store(volatile int* n, int x) {
    *n = x;
}

static inline int a_fetch_and_add(volatile int* n, int a) {
    return __sync_fetch_and_add(n, a);
}

#endif /* __ATOMIC_H_ */
