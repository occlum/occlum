#ifndef __PAL_VCPU_THREAD_H__
#define __PAL_VCPU_THREAD_H__

#include <pthread.h>

int pal_vcpu_threads_start(unsigned int num_vcpus);

int pal_vcpu_threads_stop(void);

int pal_num_vcpus;
pthread_t *pal_vcpu_threads;

#endif /* __PAL_VCPU_THREAD_H_ */
