#include <pthread.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_vcpu_thread.h"
#include "pal_log.h"
#include "pal_syscall.h"
#include "pal_thread_counter.h"
#include "errno2str.h"

static void *thread_func(void *_data) {
    sgx_enclave_id_t eid = pal_get_enclave_id();

    int ret = 0;
    sgx_status_t ecall_status = occlum_ecall_run_vcpu(eid, &ret);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: occlum_ecall_run_vcpu: %s", sgx_err);
        exit(EXIT_FAILURE);
    }
    if (ret < 0) {
        int errno_ = -ret;
        PAL_ERROR("Unexpcted error from occlum_ecall_run_vcpu: %s", errno2str(errno_));
        exit(EXIT_FAILURE);
    }

    pal_thread_counter_dec();
    return NULL;
}

int pal_vcpu_threads_start(unsigned int num_vcpus) {
    for (int vcpu_i = 0; vcpu_i < num_vcpus; vcpu_i++) {
        pal_thread_counter_inc();

        pthread_t thread;
        int ret = 0;
        if ((ret = pthread_create(&thread, NULL, thread_func, NULL))) {
            pal_thread_counter_dec();

            errno = ret;
            PAL_ERROR("Failed to start the interrupt thread: %s", errno2str(errno));
            return -1;
        }

        pthread_detach(thread);
    }
    return 0;
}

int pal_vcpu_threads_stop(void) {
    sgx_enclave_id_t eid = pal_get_enclave_id();

    int ret;
    // This ECall will make occlum_ecall_run_vcpu returns
    sgx_status_t ecall_status = occlum_ecall_shutdown_vcpus(eid, &ret);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        errno = ret;
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (ret < 0) {
        errno = ret;
        PAL_ERROR("Cannot shut down vCPUs");
        return -1;
    }
    return 0;
}
