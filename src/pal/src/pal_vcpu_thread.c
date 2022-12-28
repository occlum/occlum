#include <pthread.h>
#include <occlum_pal_api.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_vcpu_thread.h"
#include "pal_log.h"
#include "pal_syscall.h"
#include "pal_thread_counter.h"
#include "errno2str.h"

int pal_num_vcpus = 0;

pthread_t *pal_vcpu_threads = NULL;
struct occlum_pal_vcpu_data *pal_vcpu_data = NULL;

static void *thread_func(void *_data) {
    sgx_enclave_id_t eid = pal_get_enclave_id();
    struct occlum_pal_vcpu_data *vcpu_data_ptr = (struct occlum_pal_vcpu_data *)_data;

    int ret = 0;

    sgx_status_t ecall_status = occlum_ecall_run_vcpu(eid, &ret, vcpu_data_ptr);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: occlum_ecall_run_vcpu: %s", sgx_err);
        exit(EXIT_FAILURE);
    }
    if (ret < 0) {
        int errno_ = -ret;
        PAL_ERROR("Unexpected error from occlum_ecall_run_vcpu: %s", errno2str(errno_));
        exit(EXIT_FAILURE);
    }

    pal_thread_counter_dec();
    return NULL;
}

int pal_vcpu_threads_start(unsigned int num_vcpus) {
    if (num_vcpus == 0) {
        errno = EINVAL;
        return -1;
    }
    pal_num_vcpus = num_vcpus;

    pal_vcpu_threads = calloc(num_vcpus, sizeof(pthread_t));
    if (pal_vcpu_threads == NULL) {
        pal_num_vcpus = 0;
        errno = ENOMEM;
        return -1;
    }

    pal_vcpu_data = calloc(num_vcpus, sizeof(struct occlum_pal_vcpu_data));
    if (pal_vcpu_data == NULL) {
        pal_num_vcpus = 0;
        errno = ENOMEM;
        return -1;
    }

    for (int vcpu_i = 0; vcpu_i < num_vcpus; vcpu_i++) {
        pal_thread_counter_inc();

        pthread_t *thread = &pal_vcpu_threads[vcpu_i];
        pal_vcpu_data[vcpu_i].user_space_mark = 0;
        int ret = 0;
        if ((ret = pthread_create(thread, NULL, thread_func, (void *)&pal_vcpu_data[vcpu_i]))) {
            pal_thread_counter_dec();

            pal_num_vcpus = 0;
            free(pal_vcpu_threads);
            pal_vcpu_threads = NULL;

            free(pal_vcpu_data);
            pal_vcpu_data = NULL;

            errno = ret;
            PAL_ERROR("Failed to start the vCPU thread: %s", errno2str(errno));
            return -1;
        }

        // TODO: free the vCPU threads properly.
        //
        // We cannot simply detach here because it would invalidate the threads
        // in pal_vcpu_threads, which is being used by the interrupt thread to
        // iterate the vCPU threads.
        //
        // pthread_detach(*thread);
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
