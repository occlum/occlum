#include <signal.h>
#include <pthread.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_interrupt_thread.h"
#include "pal_log.h"
#include "pal_syscall.h"
#include "pal_thread_counter.h"
#include "pal_vcpu_thread.h"
#include "errno2str.h"

// #define MS                      (1000*1000L) // 1ms = 1,000,000ns
// // real-time signal 64 is used to notify interrupts
// #define INTERRUPT_SIGNAL        (64)

static pthread_t thread;
static int is_running = 0;

extern pthread_t *pal_vcpu_threads;
extern struct occlum_pal_vcpu_data *pal_vcpu_data;

static void *timer_thread(void *_data) {
    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave is not initialized yet.");
        errno = ENOENT;
        return (int *) -1;
    }

    int ecall_ret = 0;
    sgx_status_t ecall_status = occlum_ecall_timer_thread_create(eid, &ecall_ret);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return (int *) -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_run_vcpu returns %s", errno2str(errno));
        return (int *) -1;
    }

    return NULL;
}

int pal_timer_thread_start(void) {
    if (is_running) {
        errno = EEXIST;
        PAL_ERROR("The timer thread is already running: %s", errno2str(errno));
        return -1;
    }

    is_running = 1;
    pal_thread_counter_inc();

    int ret = 0;
    if ((ret = pthread_create(&thread, NULL, timer_thread, NULL))) {
        is_running = 0;
        pal_thread_counter_dec();

        errno = ret;
        PAL_ERROR("Failed to start the timer thread: %s", errno2str(errno));
        return -1;
    }
    return 0;
}

int pal_timer_thread_stop(void) {
    if (!is_running) {
        errno = ENOENT;
        return -1;
    }

    is_running = 0;
    pal_thread_counter_dec();

    int ret = 0;
    void *thread_ret = NULL;
    if ((ret = pthread_join(thread, &thread_ret))) {
        errno = ret;
        PAL_ERROR("Failed to free the timer thread: %s", errno2str(errno));
        return -1;
    }

    if ((int *)thread_ret) {
        PAL_ERROR("Timer thread exit error");
        return -1;
    }

    return 0;
}
