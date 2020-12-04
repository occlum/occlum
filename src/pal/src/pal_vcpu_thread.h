#ifndef __PAL_VCPU_THREAD_H__
#define __PAL_VCPU_THREAD_H__

int pal_vcpu_threads_start(unsigned int num_vcpus);

int pal_vcpu_threads_stop(void);

#endif /* __PAL_VCPU_THREAD_H__ */
