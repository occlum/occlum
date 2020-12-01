#include <pthread.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_interrupt_thread.h"
#include "pal_log.h"
#include "pal_syscall.h"
#include "pal_thread_counter.h"
#include "errno2str.h"

#define MS  (1000*1000L) // 1ms = 1,000,000ns

static pthread_t thread;
static int is_running = 0;

static void *thread_func(void *_data) {
    sgx_enclave_id_t eid = pal_get_enclave_id();

    int counter = 0;
    do {
        int num_broadcast_threads = 0;
        sgx_status_t ecall_status = occlum_ecall_broadcast_interrupts(eid,
                                    &num_broadcast_threads);
        if (ecall_status != SGX_SUCCESS) {
            const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
            PAL_ERROR("Failed to do ECall: occlum_ecall_broadcast_interrupts with error code 0x%x: %s",
                      ecall_status, sgx_err);
            exit(EXIT_FAILURE);
        }
        if (ecall_status == SGX_SUCCESS && num_broadcast_threads < 0) {
            int errno_ = -num_broadcast_threads;
            PAL_ERROR("Unexpcted error from cclum_ecall_broadcast_interrupts: %s", errno2str(errno_));
            exit(EXIT_FAILURE);
        }

        struct timespec timeout = { .tv_sec = 0, .tv_nsec = 25 * MS };
        counter = pal_thread_counter_wait_zero(&timeout);
    } while (counter > 0);

    return NULL;
}

int pal_interrupt_thread_start(void) {
    if (is_running) {
        errno = EEXIST;
        PAL_ERROR("The interrupt thread is already running: %s", errno2str(errno));
        return -1;
    }

    is_running = 1;
    pal_thread_counter_inc();

    int ret = 0;
    if ((ret = pthread_create(&thread, NULL, thread_func, NULL))) {
        is_running = 0;
        pal_thread_counter_dec();

        errno = ret;
        PAL_ERROR("Failed to start the interrupt thread: %s", errno2str(errno));
        return -1;
    }
    return 0;
}

int pal_interrupt_thread_stop(void) {
    if (!is_running) {
        errno = ENOENT;
        return -1;
    }

    is_running = 0;
    pal_thread_counter_dec();

    int ret = 0;
    if ((ret = pthread_join(thread, NULL))) {
        errno = ret;
        PAL_ERROR("Failed to free the interrupt thread: %s", errno2str(errno));
        return -1;
    }

    return 0;
}
